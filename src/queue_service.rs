use crate::database::{Database, QueueAttributes, QueueMetric};
use crate::message::{Message, MessageAttributeValue, ReceivedMessage};
use crate::config::QueueConfig;
use std::collections::HashMap;
use tokio_rusqlite::Result;

pub struct QueueService {
    db: Database,
}

impl QueueService {
    pub async fn new(db_path: &str) -> Result<Self> {
        let db = Database::new(db_path).await?;
        Ok(Self { db })
    }

    pub async fn create_queue(&self, queue_name: &str) -> Result<()> {
        self.db.create_queue(queue_name).await
    }

    pub async fn create_queue_with_config(&self, config: &QueueConfig) -> Result<()> {
        self.db.create_queue_with_config(config).await
    }

    pub async fn send_message(
        &self, 
        queue_name: &str, 
        body: &str,
        attributes: Option<HashMap<String, MessageAttributeValue>>,
        deduplication_id: Option<String>,
    ) -> Result<String> {
        let mut message = Message::new(queue_name.to_string(), body.to_string());
        
        if let Some(attrs) = attributes {
            message = message.with_attributes(attrs);
        }
        
        if let Some(dedup_id) = deduplication_id {
            message = message.with_deduplication_id(dedup_id);
        }
        
        let message_id = message.id.clone();
        let attributes_json = message.attributes.as_ref().map(|attrs| serde_json::to_string(attrs).unwrap());
        
        self.db.send_message(
            queue_name, 
            &message.id, 
            body,
            attributes_json.as_deref(),
            message.deduplication_id.as_deref(),
        ).await?;
        
        Ok(message_id)
    }

    pub async fn receive_message(&self, queue_name: &str) -> Result<Option<ReceivedMessage>> {
        if let Some((id, body, _created_at, attributes_json)) = self.db.receive_message(queue_name).await? {
            let attributes = if let Some(json) = attributes_json {
                serde_json::from_str(&json).ok()
            } else {
                None
            };
            
            Ok(Some(ReceivedMessage::new(id, body, attributes)))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_message(&self, receipt_handle: &str) -> Result<bool> {
        // For now, receipt_handle is the same as message ID
        self.db.delete_message(receipt_handle).await
    }

    pub async fn delete_queue(&self, queue_name: &str) -> Result<bool> {
        self.db.delete_queue(queue_name).await
    }

    pub async fn restore_message(&self, message_id: &str) -> Result<bool> {
        self.db.restore_message(message_id).await
    }

    pub async fn list_queues(&self) -> Result<Vec<(String, String)>> {
        self.db.list_queues().await
    }

    pub async fn get_queue_attributes(&self, queue_name: &str) -> Result<Option<QueueAttributes>> {
        self.db.get_queue_attributes(queue_name).await
    }

    pub async fn get_queue_messages(&self, queue_name: &str) -> Result<Vec<(String, String, String, Option<String>, u32, Option<String>, Option<String>)>> {
        self.db.get_queue_messages(queue_name).await
    }

    pub async fn get_all_queue_messages(&self, queue_name: &str) -> Result<Vec<(String, String, String, Option<String>, u32, Option<String>, Option<String>, String, Option<String>, Option<String>)>> {
        self.db.get_all_queue_messages(queue_name).await
    }

    // DLQ-aware message processing
    pub async fn receive_message_with_dlq(&self, queue_name: &str) -> Result<Option<ReceivedMessage>> {
        // Use a loop instead of recursion to handle DLQ processing
        loop {
            // Try to receive a message normally
            if let Some((id, body, _created_at, attributes_json)) = self.db.receive_message(queue_name).await? {
                let attributes = if let Some(json) = attributes_json {
                    serde_json::from_str(&json).ok()
                } else {
                    None
                };
                
                // Check if message should be moved to DLQ due to max receive count
                if let Some(queue_config) = self.db.get_queue_config(queue_name).await? {
                    // Get the current receive count from database by querying the messages again
                    if let Some((_, _, _, _, receive_count, _, _)) = self.get_message_details(&id).await? {
                        if receive_count >= queue_config.max_receive_count {
                            // Move to DLQ
                            let reason = format!("Message exceeded max receive count of {}", queue_config.max_receive_count);
                            if self.db.move_message_to_dlq(&id, &reason).await? {
                                // Message moved to DLQ, continue loop to get another message
                                continue;
                            }
                        }
                    }
                }
                
                // Message is valid, return it
                return Ok(Some(ReceivedMessage::new(id, body, attributes)));
            } else {
                // No messages available
                return Ok(None);
            }
        }
    }

    // Helper method to get message details
    async fn get_message_details(&self, _message_id: &str) -> Result<Option<(String, String, String, Option<String>, u32, Option<String>, Option<String>)>> {
        // This would need to be implemented in the database layer
        // For now, return None to avoid compilation errors
        Ok(None)
    }

    // DLQ Management operations
    pub async fn move_message_to_dlq(&self, message_id: &str, failure_reason: &str) -> Result<bool> {
        self.db.move_message_to_dlq(message_id, failure_reason).await
    }

    pub async fn get_dlq_messages(&self, _dlq_name: &str) -> Result<Vec<(String, String, String, String, Option<String>)>> {
        // Get messages from dead_letter_messages table for the specified DLQ
        // This would need implementation in database layer
        // For now return empty vector
        Ok(Vec::new())
    }

    pub async fn redrive_dlq_messages(&self, _dlq_name: &str, _source_queue: &str, _max_messages: Option<u32>) -> Result<u32> {
        // Move messages from DLQ back to source queue
        // This would need implementation in database layer
        // For now return 0
        Ok(0)
    }

    pub async fn purge_dlq(&self, _dlq_name: &str) -> Result<u32> {
        // Delete all messages from DLQ
        // This would need implementation in database layer
        // For now return 0
        Ok(0)
    }

    // Metrics operations
    pub async fn record_metric(&self, queue_name: &str, metric: &QueueMetric) -> Result<()> {
        self.db.record_queue_metric(queue_name, metric).await
    }

    // Cleanup operations
    pub async fn cleanup_expired_messages(&self, retention_config: &crate::config::RetentionConfig) -> Result<u32> {
        self.db.cleanup_expired_messages(retention_config).await
    }

    // Enhanced queue configuration
    pub async fn get_queue_config(&self, queue_name: &str) -> Result<Option<QueueConfig>> {
        self.db.get_queue_config(queue_name).await
    }

    pub async fn set_queue_attributes(&self, _queue_name: &str, _attributes: HashMap<String, String>) -> Result<()> {
        // Parse attributes and update queue configuration
        // This would update queue settings like VisibilityTimeout, MessageRetentionPeriod, etc.
        // For now, just return Ok
        Ok(())
    }
}