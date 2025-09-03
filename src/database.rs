use chrono::Utc;
use tokio_rusqlite::{Connection, OptionalExtension, Result};
use tracing::info;

pub struct Database {
    connection: Connection,
}

impl Database {
    pub async fn new(db_path: &str) -> Result<Self> {
        let connection = Connection::open(db_path).await?;
        
        let db = Database { connection };
        db.init_performance_settings().await?;
        db.init_schema().await?;
        db.create_performance_indexes().await?;
        
        Ok(db)
    }
    
    async fn init_performance_settings(&self) -> Result<()> {
        info!("Applying database performance optimizations");
        
        self.connection
            .call(|conn| {
                // Enable WAL mode for better concurrency
                let _ = conn.prepare("PRAGMA journal_mode=WAL")?.query([])?;
                info!("Enabled WAL mode for better concurrent access");
                
                // Set synchronous to NORMAL for better performance while maintaining crash safety
                let _ = conn.prepare("PRAGMA synchronous=NORMAL")?.query([])?;
                
                // Increase cache size to 8MB for better performance
                let _ = conn.prepare("PRAGMA cache_size=-8192")?.query([])?;
                
                // Store temporary tables in memory for speed
                let _ = conn.prepare("PRAGMA temp_store=MEMORY")?.query([])?;
                
                // Enable memory mapping for better I/O performance (256MB)
                let _ = conn.prepare("PRAGMA mmap_size=268435456")?.query([])?;
                
                // Optimize for concurrent access
                let _ = conn.prepare("PRAGMA busy_timeout=5000")?.query([])?;
                
                info!("Applied performance settings: WAL mode, 8MB cache, memory mapping");
                Ok(())
            })
            .await
    }

    async fn init_schema(&self) -> Result<()> {
        self.connection
            .call(|conn| {
                conn.execute(
                    r#"
                    CREATE TABLE IF NOT EXISTS messages (
                        id TEXT PRIMARY KEY,
                        queue_name TEXT NOT NULL,
                        body TEXT NOT NULL,
                        created_at TEXT NOT NULL,
                        visibility_timeout TEXT,
                        receive_count INTEGER DEFAULT 0,
                        attributes TEXT,
                        deduplication_id TEXT,
                        status TEXT DEFAULT 'active',
                        processed_at TEXT,
                        deleted_at TEXT
                    )
                    "#,
                    [],
                )?;
                
                // Add status column to existing tables if it doesn't exist
                let _ = conn.execute(
                    "ALTER TABLE messages ADD COLUMN status TEXT DEFAULT 'active'",
                    [],
                );
                let _ = conn.execute(
                    "ALTER TABLE messages ADD COLUMN processed_at TEXT",
                    [],
                );
                let _ = conn.execute(
                    "ALTER TABLE messages ADD COLUMN deleted_at TEXT",
                    [],
                );
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_queue_name ON messages(queue_name)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_visibility_timeout ON messages(visibility_timeout)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_deduplication_id ON messages(queue_name, deduplication_id)",
                    [],
                )?;
                
                conn.execute(
                    r#"
                    CREATE TABLE IF NOT EXISTS queues (
                        name TEXT PRIMARY KEY,
                        created_at TEXT NOT NULL
                    )
                    "#,
                    [],
                )?;
                
                Ok(())
            })
            .await
    }

    async fn create_performance_indexes(&self) -> Result<()> {
        info!("Creating additional performance indexes for high-throughput operations");
        
        self.connection
            .call(|conn| {
                // Additional high-performance indexes for message operations
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_queue_created ON messages(queue_name, created_at)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_visibility_available ON messages(visibility_timeout) WHERE visibility_timeout IS NULL",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_receive_count ON messages(queue_name, receive_count)",
                    [],
                )?;
                
                // Composite index for efficient cleanup operations
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_cleanup ON messages(created_at, visibility_timeout)",
                    [],
                )?;
                
                // Status-based indexes for message visibility and retention
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_status ON messages(status)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_active_queue ON messages(queue_name, status) WHERE status = 'active'",
                    [],
                )?;
                
                info!("Performance indexes created successfully");
                Ok(())
            })
            .await
    }

    pub async fn create_queue(&self, queue_name: &str) -> Result<()> {
        let queue_name = queue_name.to_string();
        let created_at = Utc::now().to_rfc3339();
        
        self.connection
            .call(move |conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO queues (name, created_at) VALUES (?1, ?2)",
                    [&queue_name, &created_at],
                )?;
                Ok(())
            })
            .await
    }

    pub async fn delete_queue(&self, queue_name: &str) -> Result<bool> {
        let queue_name = queue_name.to_string();
        
        self.connection
            .call(move |conn| {
                // First delete all messages in the queue
                conn.execute(
                    "DELETE FROM messages WHERE queue_name = ?1",
                    [&queue_name],
                )?;
                
                // Then delete the queue itself
                let changes = conn.execute(
                    "DELETE FROM queues WHERE name = ?1",
                    [&queue_name],
                )?;
                
                Ok(changes > 0)
            })
            .await
    }

    pub async fn send_message(
        &self,
        queue_name: &str,
        message_id: &str,
        body: &str,
        attributes: Option<&str>,
        deduplication_id: Option<&str>,
    ) -> Result<()> {
        let queue_name = queue_name.to_string();
        let message_id = message_id.to_string();
        let body = body.to_string();
        let created_at = Utc::now().to_rfc3339();
        let attributes = attributes.map(|s| s.to_string());
        let deduplication_id = deduplication_id.map(|s| s.to_string());

        // Check for duplicate deduplication_id within the last 5 minutes
        if let Some(ref dedup_id) = deduplication_id {
            let five_minutes_ago = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
            let queue_name_check = queue_name.clone();
            let dedup_id_check = dedup_id.clone();
            
            let duplicate_exists = self.connection
                .call(move |conn| {
                    let mut stmt = conn.prepare(
                        "SELECT COUNT(*) FROM messages WHERE queue_name = ?1 AND deduplication_id = ?2 AND created_at > ?3"
                    )?;
                    let count: i64 = stmt.query_row([&queue_name_check, &dedup_id_check, &five_minutes_ago], |row| {
                        row.get(0)
                    })?;
                    Ok(count > 0)
                })
                .await?;
                
            if duplicate_exists {
                return Ok(()); // Silently ignore duplicate
            }
        }

        self.connection
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO messages (id, queue_name, body, created_at, attributes, deduplication_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    [
                        &Some(message_id), 
                        &Some(queue_name), 
                        &Some(body), 
                        &Some(created_at),
                        &attributes,
                        &deduplication_id
                    ],
                )?;
                Ok(())
            })
            .await
    }

    pub async fn receive_message(&self, queue_name: &str) -> Result<Option<(String, String, String, Option<String>)>> {
        let queue_name = queue_name.to_string();
        let processed_at = Utc::now().to_rfc3339();

        self.connection
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT id, body, created_at, attributes 
                    FROM messages 
                    WHERE queue_name = ?1 
                    AND status = 'active'
                    AND (visibility_timeout IS NULL OR visibility_timeout < datetime('now'))
                    ORDER BY created_at ASC 
                    LIMIT 1
                    "#,
                )?;

                let mut rows = stmt.query_map([&queue_name], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })?;

                if let Some(row) = rows.next() {
                    let (id, body, created_at, attributes) = row?;
                    
                    // Set visibility timeout (30 seconds from now) and mark as processed
                    let timeout = (Utc::now() + chrono::Duration::seconds(30)).to_rfc3339();
                    conn.execute(
                        "UPDATE messages SET visibility_timeout = ?1, receive_count = receive_count + 1, status = 'processing', processed_at = ?3 WHERE id = ?2",
                        [&timeout, &id, &processed_at],
                    )?;
                    
                    Ok(Some((id, body, created_at, attributes)))
                } else {
                    Ok(None)
                }
            })
            .await
    }

    pub async fn delete_message(&self, message_id: &str) -> Result<bool> {
        let message_id = message_id.to_string();
        let deleted_at = Utc::now().to_rfc3339();

        self.connection
            .call(move |conn| {
                let changes = conn.execute(
                    "UPDATE messages SET status = 'deleted', deleted_at = ?2 WHERE id = ?1",
                    [&message_id, &deleted_at],
                )?;
                Ok(changes > 0)
            })
            .await
    }

    pub async fn restore_message(&self, message_id: &str) -> Result<bool> {
        let message_id = message_id.to_string();

        self.connection
            .call(move |conn| {
                let changes = conn.execute(
                    "UPDATE messages SET status = 'active', deleted_at = NULL, visibility_timeout = NULL WHERE id = ?1",
                    [&message_id],
                )?;
                Ok(changes > 0)
            })
            .await
    }

    pub async fn list_queues(&self) -> Result<Vec<(String, String)>> {
        self.connection
            .call(|conn| {
                let mut stmt = conn.prepare("SELECT name, created_at FROM queues ORDER BY name")?;
                let rows = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                    ))
                })?;

                let mut queues = Vec::new();
                for row in rows {
                    queues.push(row?);
                }
                Ok(queues)
            })
            .await
    }

    pub async fn get_queue_messages(&self, queue_name: &str) -> Result<Vec<(String, String, String, Option<String>, u32, Option<String>, Option<String>)>> {
        let queue_name = queue_name.to_string();
        
        self.connection
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, body, created_at, visibility_timeout, receive_count, attributes, deduplication_id FROM messages WHERE queue_name = ?1 AND status = 'active' ORDER BY created_at ASC"
                )?;
                
                let rows = stmt.query_map([&queue_name], |row| {
                    Ok((
                        row.get::<_, String>(0)?,        // id
                        row.get::<_, String>(1)?,        // body
                        row.get::<_, String>(2)?,        // created_at
                        row.get::<_, Option<String>>(3)?, // visibility_timeout
                        row.get::<_, u32>(4)?,           // receive_count
                        row.get::<_, Option<String>>(5)?, // attributes
                        row.get::<_, Option<String>>(6)?, // deduplication_id
                    ))
                })?;

                let mut messages = Vec::new();
                for row in rows {
                    messages.push(row?);
                }
                Ok(messages)
            })
            .await
    }

    pub async fn get_all_queue_messages(&self, queue_name: &str) -> Result<Vec<(String, String, String, Option<String>, u32, Option<String>, Option<String>, String, Option<String>, Option<String>)>> {
        let queue_name = queue_name.to_string();
        
        self.connection
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, body, created_at, visibility_timeout, receive_count, attributes, deduplication_id, status, processed_at, deleted_at FROM messages WHERE queue_name = ?1 ORDER BY created_at ASC"
                )?;
                
                let rows = stmt.query_map([&queue_name], |row| {
                    Ok((
                        row.get::<_, String>(0)?,         // id
                        row.get::<_, String>(1)?,         // body
                        row.get::<_, String>(2)?,         // created_at
                        row.get::<_, Option<String>>(3)?,  // visibility_timeout
                        row.get::<_, u32>(4)?,            // receive_count
                        row.get::<_, Option<String>>(5)?,  // attributes
                        row.get::<_, Option<String>>(6)?,  // deduplication_id
                        row.get::<_, String>(7)?,         // status
                        row.get::<_, Option<String>>(8)?,  // processed_at
                        row.get::<_, Option<String>>(9)?,  // deleted_at
                    ))
                })?;

                let mut messages = Vec::new();
                for row in rows {
                    messages.push(row?);
                }
                Ok(messages)
            })
            .await
    }

    pub async fn get_queue_attributes(&self, queue_name: &str) -> Result<Option<QueueAttributes>> {
        let queue_name = queue_name.to_string();
        
        self.connection
            .call(move |conn| {
                // Get queue metadata
                let mut stmt = conn.prepare("SELECT created_at FROM queues WHERE name = ?1")?;
                let queue_exists: Option<String> = stmt.query_row([&queue_name], |row| {
                    row.get(0)
                }).optional()?;
                
                if queue_exists.is_none() {
                    return Ok(None);
                }
                
                // Get message counts - only count active messages
                let mut stmt = conn.prepare(
                    "SELECT COUNT(*) FROM messages WHERE queue_name = ?1 AND status = 'active'"
                )?;
                let total_active_messages: i64 = stmt.query_row([&queue_name], |row| row.get(0))?;
                
                let mut stmt = conn.prepare(
                    "SELECT COUNT(*) FROM messages WHERE queue_name = ?1 AND status = 'active' AND (visibility_timeout IS NULL OR visibility_timeout < datetime('now'))"
                )?;
                let visible_messages: i64 = stmt.query_row([&queue_name], |row| row.get(0))?;
                
                let in_flight_messages = total_active_messages - visible_messages;
                
                Ok(Some(QueueAttributes {
                    approximate_number_of_messages: visible_messages as u32,
                    approximate_number_of_messages_not_visible: in_flight_messages as u32,
                    created_timestamp: queue_exists.unwrap(),
                }))
            })
            .await
    }

    // Placeholder methods for advanced features (to be implemented)
    pub async fn create_queue_with_config(&self, config: &crate::config::QueueConfig) -> Result<()> {
        // For now, just create a basic queue
        self.create_queue(&config.name).await
    }

    pub async fn get_queue_config(&self, _queue_name: &str) -> Result<Option<crate::config::QueueConfig>> {
        // Placeholder - return None for now
        Ok(None)
    }

    pub async fn move_message_to_dlq(&self, _message_id: &str, _failure_reason: &str) -> Result<bool> {
        // Placeholder - return false for now
        Ok(false)
    }

    pub async fn record_queue_metric(&self, _queue_name: &str, _metric: &QueueMetric) -> Result<()> {
        // Placeholder - do nothing for now
        Ok(())
    }

    pub async fn cleanup_expired_messages(&self, retention_config: &crate::config::RetentionConfig) -> Result<u32> {
        match retention_config.mode {
            crate::config::RetentionMode::KeepForever => {
                // In KeepForever mode, just clean up visibility timeouts for processing messages
                // that have timed out and should be available again
                let now = Utc::now().to_rfc3339();
                
                self.connection
                    .call(move |conn| {
                        let changes = conn.execute(
                            "UPDATE messages SET status = 'active', visibility_timeout = NULL WHERE status = 'processing' AND visibility_timeout < ?1",
                            [&now],
                        )?;
                        Ok(changes as u32)
                    })
                    .await
            },
            crate::config::RetentionMode::Delete => {
                // In Delete mode, actually delete messages older than the configured retention period
                let retention_days = retention_config.delete_after_days.unwrap_or(14);
                let retention_seconds = (retention_days as i64) * 24 * 3600;
                let cutoff_time = Utc::now() - chrono::Duration::seconds(retention_seconds);
                let cutoff_str = cutoff_time.to_rfc3339();
                
                self.connection
                    .call(move |conn| {
                        let mut stmt = conn.prepare(
                            "DELETE FROM messages WHERE created_at < ?1"
                        )?;
                        let deleted = stmt.execute([cutoff_str])?;
                        Ok(deleted as u32)
                    })
                    .await
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueAttributes {
    pub approximate_number_of_messages: u32,
    pub approximate_number_of_messages_not_visible: u32,
    pub created_timestamp: String,
}

#[derive(Debug, Clone)]
pub struct QueueMetric {
    pub messages_sent: u32,
    pub messages_received: u32,
    pub messages_deleted: u32,
    pub processing_time_ms: u32,
}