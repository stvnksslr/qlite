use crate::database::{Database, QueueAttributes};
use crate::message::{Message, MessageAttributeValue, ReceivedMessage};
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

    pub async fn list_queues(&self) -> Result<Vec<(String, String)>> {
        self.db.list_queues().await
    }

    pub async fn get_queue_attributes(&self, queue_name: &str) -> Result<Option<QueueAttributes>> {
        self.db.get_queue_attributes(queue_name).await
    }
}