use crate::database::{Database, QueueAttributes, QueueMetric};
use crate::message::{Message, MessageAttributeValue, ReceivedMessage};
use crate::config::QueueConfig;
use std::collections::HashMap;
use tokio_rusqlite::Result;
use tokio::sync::broadcast;
use std::sync::Arc;

pub struct QueueService {
    db: Database,
    // Notification system for long polling
    message_notifiers: Arc<tokio::sync::RwLock<HashMap<String, broadcast::Sender<()>>>>,
}

impl QueueService {
    pub async fn new(db_path: &str) -> Result<Self> {
        let db = Database::new(db_path).await?;
        Ok(Self { 
            db,
            message_notifiers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        })
    }

    pub async fn create_queue(&self, queue_name: &str) -> Result<()> {
        // Check if this is a FIFO queue based on naming convention
        let is_fifo = queue_name.ends_with(".fifo");
        
        // Validate FIFO queue name
        if is_fifo && queue_name.len() <= 5 {
            return Err(tokio_rusqlite::Error::Rusqlite(
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                    Some("FIFO queue name must be more than just .fifo suffix".to_string())
                )
            ));
        }
        
        // Create the queue
        self.db.create_queue(queue_name).await?;
        
        // If it's a FIFO queue, set up FIFO-specific configuration
        if is_fifo {
            let mut config = crate::config::QueueConfig::default();
            config.name = queue_name.to_string();
            config.is_fifo = true;
            config.content_based_deduplication = true; // Default for FIFO
            
            self.db.create_queue_with_config(&config).await?;
        }
        
        Ok(())
    }

    #[allow(dead_code)]
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
        
        // Notify any waiting long polling requests
        self.notify_message_arrival(queue_name).await;
        
        Ok(message_id)
    }

    // Internal method to notify waiting long polling requests
    async fn notify_message_arrival(&self, queue_name: &str) {
        let notifiers = self.message_notifiers.read().await;
        if let Some(sender) = notifiers.get(queue_name) {
            // Send notification (ignore if no receivers)
            let _ = sender.send(());
        }
    }

    // Internal method to get or create a notification receiver for long polling
    async fn get_notification_receiver(&self, queue_name: &str) -> broadcast::Receiver<()> {
        let mut notifiers = self.message_notifiers.write().await;
        let sender = notifiers.entry(queue_name.to_string())
            .or_insert_with(|| {
                let (sender, _) = broadcast::channel(100); // Buffer size for notifications
                sender
            });
        sender.subscribe()
    }

    // Cleanup method to remove unused notification channels (prevents memory leaks)
    #[allow(dead_code)]
    async fn cleanup_notification_channels(&self) {
        let mut notifiers = self.message_notifiers.write().await;
        notifiers.retain(|_queue_name, sender| {
            sender.receiver_count() > 0 // Keep only channels with active receivers
        });
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

    #[allow(dead_code)]
    pub async fn get_queue_messages(&self, queue_name: &str) -> Result<Vec<(String, String, String, Option<String>, u32, Option<String>, Option<String>)>> {
        self.db.get_queue_messages(queue_name).await
    }

    pub async fn get_all_queue_messages(&self, queue_name: &str) -> Result<Vec<(String, String, String, Option<String>, u32, Option<String>, Option<String>, String, Option<String>, Option<String>)>> {
        self.db.get_all_queue_messages(queue_name).await
    }

    // DLQ-aware message processing
    #[allow(dead_code)]
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
                    if let Some((_, _, _, _, receive_count, _, _)) = self.get_message_details(&id).await?
                        && Some(receive_count) >= queue_config.max_receive_count {
                            // Move to DLQ
                            let reason = format!("Message exceeded max receive count of {}", queue_config.max_receive_count.unwrap_or(0));
                            if self.db.move_message_to_dlq(&id, &reason).await? {
                                // Message moved to DLQ, continue loop to get another message
                                continue;
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
    #[allow(dead_code)]
    async fn get_message_details(&self, _message_id: &str) -> Result<Option<(String, String, String, Option<String>, u32, Option<String>, Option<String>)>> {
        // This would need to be implemented in the database layer
        // For now, return None to avoid compilation errors
        Ok(None)
    }

    // DLQ Management operations
    #[allow(dead_code)]
    pub async fn move_message_to_dlq(&self, message_id: &str, failure_reason: &str) -> Result<bool> {
        self.db.move_message_to_dlq(message_id, failure_reason).await
    }

    #[allow(dead_code)]
    pub async fn get_dlq_messages(&self, dlq_name: &str) -> Result<Vec<(String, String, String, String, Option<String>)>> {
        // Get messages from dead_letter_messages table for the specified DLQ
        self.db.get_dlq_messages(dlq_name).await
    }

    #[allow(dead_code)]
    pub async fn redrive_dlq_messages(&self, dlq_name: &str, source_queue: &str, max_messages: Option<u32>) -> Result<u32> {
        // Move messages from DLQ back to source queue
        self.db.redrive_dlq_messages(dlq_name, source_queue, max_messages).await
    }

    #[allow(dead_code)]
    pub async fn purge_dlq(&self, dlq_name: &str) -> Result<u32> {
        // Delete all messages from DLQ
        self.db.purge_dlq(dlq_name).await
    }

    // Metrics operations
    #[allow(dead_code)]
    pub async fn record_metric(&self, queue_name: &str, metric: &QueueMetric) -> Result<()> {
        self.db.record_queue_metric(queue_name, metric).await
    }

    // Cleanup operations
    pub async fn cleanup_expired_messages(&self, retention_config: &crate::config::RetentionConfig) -> Result<u32> {
        self.db.cleanup_expired_messages(retention_config).await
    }

    // Background cleanup task for production performance

    // Enhanced queue configuration
    #[allow(dead_code)]
    pub async fn get_queue_config(&self, queue_name: &str) -> Result<Option<QueueConfig>> {
        self.db.get_queue_config(queue_name).await
    }

    pub async fn set_queue_attributes(&self, queue_name: &str, attributes: HashMap<String, String>) -> Result<()> {
        // Delegate to database layer for actual implementation
        self.db.set_queue_attributes(queue_name, &attributes).await
    }

    pub async fn send_message_enhanced(
        &self,
        queue_name: &str,
        body: &str,
        attributes: Option<HashMap<String, MessageAttributeValue>>,
        deduplication_id: Option<String>,
        delay_seconds: u32,
    ) -> Result<String> {
        // For FIFO queues, MessageGroupId is required but we'll use a default for backwards compatibility
        self.send_message_enhanced_with_group(queue_name, body, attributes, deduplication_id, delay_seconds, None).await
    }

    pub async fn send_message_enhanced_with_group(
        &self,
        queue_name: &str,
        body: &str,
        attributes: Option<HashMap<String, MessageAttributeValue>>,
        deduplication_id: Option<String>,
        delay_seconds: u32,
        message_group_id: Option<String>,
    ) -> Result<String> {
        let mut message = Message::new(queue_name.to_string(), body.to_string());
        
        if let Some(attrs) = attributes {
            message = message.with_attributes(attrs);
        }
        
        if let Some(dedup_id) = deduplication_id {
            message = message.with_deduplication_id(dedup_id);
        }

        if delay_seconds > 0 {
            message = message.with_delay_seconds(delay_seconds);
        }
        
        if let Some(group_id) = message_group_id {
            message = message.with_message_group_id(group_id);
        }
        
        let message_id = message.id.clone();
        let attributes_json = message.attributes.as_ref().map(|attrs| serde_json::to_string(attrs).unwrap());
        
        // Use the enhanced send_message_with_delay method to support DelaySeconds and FIFO
        let delay_until_str = message.delay_until.map(|dt| dt.to_rfc3339());
        self.db.send_message_with_delay_and_group(
            queue_name, 
            &message.id, 
            body,
            attributes_json.as_deref(),
            message.deduplication_id.as_deref(),
            delay_until_str.as_deref(),
            message.message_group_id.as_deref(),
        ).await?;
        
        // Notify any waiting long polling requests
        self.notify_message_arrival(queue_name).await;
        
        Ok(message_id)
    }

    pub async fn receive_messages_enhanced(&self, queue_name: &str, max_messages: u32, wait_time_seconds: u32) -> Result<Vec<ReceivedMessage>> {
        let mut messages = Vec::new();
        
        // First, try to get available messages immediately
        for _ in 0..max_messages {
            if let Some(message) = self.receive_message(queue_name).await? {
                messages.push(message);
            } else {
                break;
            }
        }
        
        // If we have messages or no wait time requested, return immediately
        if !messages.is_empty() || wait_time_seconds == 0 {
            return Ok(messages);
        }
        
        // Implement efficient long polling with notifications
        let wait_duration = std::time::Duration::from_secs(std::cmp::min(wait_time_seconds, 20) as u64);
        let mut notification_receiver = self.get_notification_receiver(queue_name).await;
        
        // Use tokio::select! to wait for either a timeout or a notification
        let timeout_future = tokio::time::sleep(wait_duration);
        tokio::pin!(timeout_future);
        
        loop {
            tokio::select! {
                // Timeout reached
                _ = &mut timeout_future => {
                    break;
                }
                // Notification received (new message might be available)
                result = notification_receiver.recv() => {
                    match result {
                        Ok(_) => {
                            // Check for messages again
                            for _ in 0..(max_messages - messages.len() as u32) {
                                if let Some(message) = self.receive_message(queue_name).await? {
                                    messages.push(message);
                                    if messages.len() >= max_messages as usize {
                                        return Ok(messages);
                                    }
                                } else {
                                    break;
                                }
                            }
                            
                            // If we got messages, return them
                            if !messages.is_empty() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Channel lagged, try to get messages anyway
                            for _ in 0..(max_messages - messages.len() as u32) {
                                if let Some(message) = self.receive_message(queue_name).await? {
                                    messages.push(message);
                                    if messages.len() >= max_messages as usize {
                                        return Ok(messages);
                                    }
                                } else {
                                    break;
                                }
                            }
                            
                            if !messages.is_empty() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Channel closed, fall back to periodic polling
                            break;
                        }
                    }
                }
            }
        }
        
        Ok(messages)
    }

    // Batch operations for Phase 2
    pub async fn send_messages_batch(
        &self,
        entries: Vec<(String, String, String, Option<std::collections::HashMap<String, MessageAttributeValue>>, Option<String>, u32)>
    ) -> Result<Vec<std::result::Result<String, String>>> {
        // Track which queues need notifications
        let mut queues_to_notify = std::collections::HashSet::new();
        
        // Transform queue service entries to database format
        let db_entries: Vec<(String, String, String, Option<String>, Option<String>, Option<String>)> = entries
            .into_iter()
            .map(|(queue_name, message_id, body, attributes, deduplication_id, delay_seconds)| {
                queues_to_notify.insert(queue_name.clone());
                let attributes_json = attributes.map(|attrs| serde_json::to_string(&attrs).unwrap());
                let delay_until = if delay_seconds > 0 {
                    Some((chrono::Utc::now() + chrono::Duration::seconds(delay_seconds as i64)).to_rfc3339())
                } else {
                    None
                };
                
                (queue_name, message_id.clone(), body, attributes_json, deduplication_id, delay_until)
            })
            .collect();
        
        let results = self.db.send_messages_batch(db_entries).await?;
        
        // Notify all affected queues
        for queue_name in queues_to_notify {
            self.notify_message_arrival(&queue_name).await;
        }
        
        // Transform database results back to service layer format
        let mut service_results = Vec::new();
        for (_i, result) in results.into_iter().enumerate() {
            match result {
                Ok(_) => service_results.push(Ok("Success".to_string())), // In real SQS, this would be MessageId
                Err(e) => service_results.push(Err(e))
            }
        }
        
        Ok(service_results)
    }

    pub async fn delete_messages_batch(&self, message_ids: Vec<String>) -> Result<Vec<std::result::Result<bool, String>>> {
        self.db.delete_messages_batch(message_ids).await
    }

    pub async fn receive_messages_batch(&self, queue_name: &str, max_messages: u32) -> Result<Vec<ReceivedMessage>> {
        let db_messages = self.db.receive_messages_batch(queue_name, max_messages).await?;
        
        let mut messages = Vec::new();
        for (id, body, _created_at, attributes_json) in db_messages {
            let attributes = if let Some(json) = attributes_json {
                serde_json::from_str(&json).ok()
            } else {
                None
            };
            
            messages.push(ReceivedMessage::new(id, body, attributes));
        }
        
        Ok(messages)
    }
}