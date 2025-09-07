use chrono::Utc;
use tokio_rusqlite::{Connection, OptionalExtension, Result};
use tracing::info;

// Type aliases to fix clippy warnings
pub type DelayedMessageTuple = (String, String, String, Option<String>, Option<String>, Option<String>);

// Struct to fix too_many_arguments warning
#[derive(Debug)]
pub struct SendMessageParams<'a> {
    pub queue_name: &'a str,
    pub message_id: &'a str,
    pub body: &'a str,
    pub attributes: Option<&'a str>,
    pub deduplication_id: Option<&'a str>,
    pub delay_until: Option<&'a str>,
    pub message_group_id: Option<&'a str>,
}

#[derive(Clone)]
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

                // Add DelaySeconds support columns
                let _ = conn.execute(
                    "ALTER TABLE messages ADD COLUMN delay_until TEXT",
                    [],
                );
                let _ = conn.execute(
                    "ALTER TABLE messages ADD COLUMN message_group_id TEXT",
                    [],
                );
                let _ = conn.execute(
                    "ALTER TABLE messages ADD COLUMN sequence_number INTEGER",
                    [],
                );

                // Create queue_config table for SetQueueAttributes support
                conn.execute(
                    r#"
                    CREATE TABLE IF NOT EXISTS queue_config (
                        name TEXT PRIMARY KEY,
                        is_fifo BOOLEAN DEFAULT FALSE,
                        content_based_deduplication BOOLEAN DEFAULT FALSE,
                        visibility_timeout_seconds INTEGER DEFAULT 30,
                        message_retention_period_seconds INTEGER DEFAULT 1209600,
                        max_receive_count INTEGER,
                        dead_letter_target_arn TEXT,
                        delay_seconds INTEGER DEFAULT 0,
                        receive_message_wait_time_seconds INTEGER DEFAULT 0
                    )
                    "#,
                    [],
                )?;

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

                // Create dead_letter_messages table for DLQ support
                conn.execute(
                    r#"
                    CREATE TABLE IF NOT EXISTS dead_letter_messages (
                        id TEXT PRIMARY KEY,
                        original_queue_name TEXT NOT NULL,
                        dlq_name TEXT NOT NULL,
                        failure_reason TEXT NOT NULL,
                        moved_at TEXT NOT NULL,
                        original_message_data TEXT NOT NULL,
                        original_body TEXT NOT NULL,
                        original_attributes TEXT,
                        receive_count INTEGER DEFAULT 0,
                        original_created_at TEXT NOT NULL
                    )
                    "#,
                    [],
                )?;

                // Create indexes for DLQ operations
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_dlq_messages_dlq_name ON dead_letter_messages(dlq_name)",
                    [],
                )?;

                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_dlq_messages_original_queue ON dead_letter_messages(original_queue_name)",
                    [],
                )?;

                // Add receive_count column to messages table for DLQ functionality
                let _ = conn.execute(
                    "ALTER TABLE messages ADD COLUMN receive_count INTEGER DEFAULT 0",
                    [],
                );

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

                // Add indexes for DelaySeconds and FIFO support
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_delay ON messages(queue_name, delay_until)",
                    [],
                )?;

                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_fifo_order ON messages(queue_name, message_group_id, sequence_number)",
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
                conn.execute("DELETE FROM messages WHERE queue_name = ?1", [&queue_name])?;

                // Then delete the queue itself
                let changes = conn.execute("DELETE FROM queues WHERE name = ?1", [&queue_name])?;

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

    pub async fn receive_message(
        &self,
        queue_name: &str,
    ) -> Result<Option<(String, String, String, Option<String>)>> {
        let queue_name = queue_name.to_string();
        let processed_at = Utc::now().to_rfc3339();

        self.connection
            .call(move |conn| {
                // Check if this is a FIFO queue to determine ordering
                let queue_config_result: Option<(bool,)> = conn.prepare(
                    "SELECT is_fifo FROM queue_config WHERE name = ?1"
                )?.query_row([&queue_name], |row| {
                    Ok((row.get::<_, i32>(0)? != 0,))
                }).optional()?;

                let is_fifo = queue_config_result.map(|(fifo,)| fifo).unwrap_or(false);

                let mut stmt = if is_fifo {
                    // For FIFO queues, order by sequence_number for strict FIFO ordering
                    conn.prepare(
                        r#"
                        SELECT id, body, created_at, attributes
                        FROM messages
                        WHERE queue_name = ?1
                        AND status = 'active'
                        AND (visibility_timeout IS NULL OR visibility_timeout < datetime('now'))
                        AND (delay_until IS NULL OR delay_until < datetime('now'))
                        ORDER BY sequence_number ASC
                        LIMIT 1
                        "#,
                    )?
                } else {
                    // For standard queues, order by created_at
                    conn.prepare(
                        r#"
                        SELECT id, body, created_at, attributes
                        FROM messages
                        WHERE queue_name = ?1
                        AND status = 'active'
                        AND (visibility_timeout IS NULL OR visibility_timeout < datetime('now'))
                        AND (delay_until IS NULL OR delay_until < datetime('now'))
                        ORDER BY created_at ASC
                        LIMIT 1
                        "#,
                    )?
                };

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

                    // Get current receive count and queue configuration
                    let current_receive_count: i32 = conn.prepare(
                        "SELECT receive_count FROM messages WHERE id = ?1"
                    )?.query_row([&id], |row| row.get(0))?;

                    // Check for DLQ configuration
                    let queue_config = conn.prepare(
                        "SELECT max_receive_count, dead_letter_target_arn FROM queue_config WHERE name = ?1"
                    )?.query_row([&queue_name], |row| {
                        Ok((row.get::<_, Option<i32>>(0)?, row.get::<_, Option<String>>(1)?))
                    }).optional()?;

                    let new_receive_count = current_receive_count + 1;

                    // Check if message should be moved to DLQ
                    if let Some((Some(max_receive_count), Some(_dlq_arn))) = queue_config
                        && new_receive_count > max_receive_count {
                            // Move to DLQ instead of returning the message
                            let _reason = format!("Message exceeded max receive count of {}", max_receive_count);

                            // Get message details for DLQ move
                            let _message_details = conn.prepare(
                                "SELECT queue_name, body, created_at, attributes FROM messages WHERE id = ?1"
                            )?.query_row([&id], |row| {
                                Ok((
                                    row.get::<_, String>(0)?,
                                    row.get::<_, String>(1)?,
                                    row.get::<_, String>(2)?,
                                    row.get::<_, Option<String>>(3)?,
                                ))
                            })?;

                            // This will be handled by a separate call - for now mark as failed and let DLQ processing handle it
                            conn.execute(
                                "UPDATE messages SET status = 'dlq_pending', receive_count = ?2 WHERE id = ?1",
                                [&id, &new_receive_count.to_string()],
                            )?;

                            // Return None to indicate message was moved to DLQ processing
                            return Ok(None);
                        }

                    // Set visibility timeout (30 seconds from now) and increment receive count
                    let timeout = (Utc::now() + chrono::Duration::seconds(30)).to_rfc3339();
                    conn.execute(
                        "UPDATE messages SET visibility_timeout = ?1, receive_count = ?2, status = 'processing', processed_at = ?3 WHERE id = ?4",
                        [&timeout, &new_receive_count.to_string(), &processed_at, &id],
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
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;

                let mut queues = Vec::new();
                for row in rows {
                    queues.push(row?);
                }
                Ok(queues)
            })
            .await
    }

    #[allow(dead_code)]
    pub async fn get_queue_messages(
        &self,
        queue_name: &str,
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            Option<String>,
            u32,
            Option<String>,
            Option<String>,
        )>,
    > {
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

    pub async fn get_all_queue_messages(
        &self,
        queue_name: &str,
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            Option<String>,
            u32,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
        )>,
    > {
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

    // FIFO queue creation with configuration
    pub async fn create_queue_with_config(
        &self,
        config: &crate::config::QueueConfig,
    ) -> Result<()> {
        // Create the basic queue first
        self.create_queue(&config.name).await?;

        // Store the configuration in queue_config table
        let config_name = config.name.clone();
        let is_fifo = config.is_fifo;
        let content_based_dedup = config.content_based_deduplication;
        let visibility_timeout = config.visibility_timeout_seconds as i32;
        let retention_period = config.message_retention_period_seconds as i32;
        let max_receive_count = config.max_receive_count.map(|v| v as i32);
        let delay_seconds = config.delay_seconds as i32;
        let wait_time = config.receive_message_wait_time_seconds as i32;
        let dlq_arn = config.dead_letter_target_arn.clone();

        self.connection
            .call(move |conn| {
                conn.execute(
                    r#"
                    INSERT OR REPLACE INTO queue_config
                    (name, is_fifo, content_based_deduplication, visibility_timeout_seconds,
                     message_retention_period_seconds, max_receive_count, dead_letter_target_arn,
                     delay_seconds, receive_message_wait_time_seconds)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                    "#,
                    rusqlite::params![
                        config_name,
                        is_fifo as i32,
                        content_based_dedup as i32,
                        visibility_timeout,
                        retention_period,
                        max_receive_count,
                        dlq_arn,
                        delay_seconds,
                        wait_time
                    ],
                )?;
                Ok(())
            })
            .await
    }

    pub async fn get_queue_config(
        &self,
        queue_name: &str,
    ) -> Result<Option<crate::config::QueueConfig>> {
        let queue_name = queue_name.to_string();

        self.connection
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT name, is_fifo, content_based_deduplication, visibility_timeout_seconds,
                           message_retention_period_seconds, max_receive_count, dead_letter_target_arn,
                           delay_seconds, receive_message_wait_time_seconds
                    FROM queue_config WHERE name = ?1
                    "#,
                )?;

                let result = stmt.query_row([&queue_name], |row| {
                    let max_receive_count: Option<i32> = row.get::<_, Option<i32>>(5)?;
                    let dead_letter_target_arn: Option<String> = row.get::<_, Option<String>>(6)?;

                    Ok(crate::config::QueueConfig {
                        name: row.get::<_, String>(0)?,
                        is_fifo: row.get::<_, i32>(1)? != 0,
                        content_based_deduplication: row.get::<_, i32>(2)? != 0,
                        visibility_timeout_seconds: row.get::<_, i32>(3)? as u32,
                        message_retention_period_seconds: row.get::<_, i32>(4)? as u32,
                        max_receive_count: max_receive_count.map(|v| v as u32),
                        dead_letter_target_arn,
                        delay_seconds: row.get::<_, i32>(7)? as u32,
                        receive_message_wait_time_seconds: row.get::<_, i32>(8)? as u32,
                    })
                }).optional()?;

                Ok(result)
            })
            .await
    }

    #[allow(dead_code)]
    pub async fn move_message_to_dlq(
        &self,
        message_id: &str,
        failure_reason: &str,
    ) -> Result<bool> {
        let message_id = message_id.to_string();
        let failure_reason = failure_reason.to_string();
        let moved_at = Utc::now().to_rfc3339();

        self.connection
            .call(move |conn| {
                // First, get the message details and queue configuration
                let message_result = conn.prepare(
                    "SELECT queue_name, body, created_at, attributes, receive_count FROM messages WHERE id = ?1 AND status != 'deleted'"
                )?.query_row([&message_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,  // queue_name
                        row.get::<_, String>(1)?,  // body
                        row.get::<_, String>(2)?,  // created_at
                        row.get::<_, Option<String>>(3)?,  // attributes
                        row.get::<_, i32>(4)?      // receive_count
                    ))
                });

                if let Ok((queue_name, body, created_at, attributes, receive_count)) = message_result {
                    // Get DLQ configuration from queue_config
                    if let Some(dlq_arn) = conn.prepare(
                        "SELECT dead_letter_target_arn FROM queue_config WHERE name = ?1"
                    )?.query_row([&queue_name], |row| {
                        row.get::<_, Option<String>>(0)
                    }).optional()? {
                        if let Some(dlq_name) = dlq_arn {
                            // Extract DLQ name from ARN (simplified - assume it's just the queue name for now)
                            let dlq_queue_name = dlq_name.split('/').next_back().unwrap_or(&dlq_name);

                            // Create JSON representation of original message data
                            let original_message_data = serde_json::json!({
                                "messageId": message_id,
                                "body": body,
                                "attributes": attributes,
                                "createdAt": created_at,
                                "receiveCount": receive_count
                            }).to_string();

                            // Insert into dead_letter_messages table
                            conn.execute(
                                r#"
                                INSERT INTO dead_letter_messages
                                (id, original_queue_name, dlq_name, failure_reason, moved_at,
                                 original_message_data, original_body, original_attributes,
                                 receive_count, original_created_at)
                                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                                "#,
                                [
                                    &message_id,
                                    &queue_name,
                                    &dlq_queue_name.to_string(),
                                    &failure_reason,
                                    &moved_at,
                                    &original_message_data,
                                    &body,
                                    &attributes.unwrap_or_else(|| "".to_string()),
                                    &receive_count.to_string(),
                                    &created_at,
                                ]
                            )?;

                            // Remove original message from messages table
                            conn.execute(
                                "DELETE FROM messages WHERE id = ?1",
                                [&message_id]
                            )?;

                            Ok(true)
                        } else {
                            // No DLQ configured for this queue
                            Ok(false)
                        }
                    } else {
                        // No queue configuration found
                        Ok(false)
                    }
                } else {
                    // Message not found or already deleted
                    Ok(false)
                }
            })
            .await
    }

    pub async fn get_dlq_messages(
        &self,
        dlq_name: &str,
    ) -> Result<Vec<(String, String, String, String, Option<String>)>> {
        let dlq_name = dlq_name.to_string();

        self.connection
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT id, original_body, moved_at, failure_reason, original_attributes
                    FROM dead_letter_messages
                    WHERE dlq_name = ?1
                    ORDER BY moved_at DESC
                    "#,
                )?;

                let rows = stmt.query_map([&dlq_name], |row| {
                    Ok((
                        row.get::<_, String>(0)?,         // id
                        row.get::<_, String>(1)?,         // original_body
                        row.get::<_, String>(2)?,         // moved_at
                        row.get::<_, String>(3)?,         // failure_reason
                        row.get::<_, Option<String>>(4)?, // original_attributes
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

    pub async fn redrive_dlq_messages(
        &self,
        dlq_name: &str,
        source_queue: &str,
        max_messages: Option<u32>,
    ) -> Result<u32> {
        let dlq_name = dlq_name.to_string();
        let source_queue = source_queue.to_string();
        let limit = max_messages.unwrap_or(10); // AWS default

        self.connection
            .call(move |conn| {
                // Get messages from DLQ to redrive
                let mut stmt = conn.prepare(
                    r#"
                    SELECT id, original_body, original_attributes, original_created_at
                    FROM dead_letter_messages
                    WHERE dlq_name = ?1 AND original_queue_name = ?2
                    LIMIT ?3
                    "#
                )?;

                let rows = stmt.query_map([&dlq_name, &source_queue, &limit.to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,        // id
                        row.get::<_, String>(1)?,        // original_body
                        row.get::<_, Option<String>>(2)?, // original_attributes
                        row.get::<_, String>(3)?,        // original_created_at
                    ))
                })?;

                let mut redriven_count = 0;
                let now = chrono::Utc::now().to_rfc3339();

                for row in rows {
                    let (message_id, body, attributes, _created_at) = row?;

                    // Insert message back into original queue with new ID and timestamp
                    let new_message_id = uuid::Uuid::new_v4().to_string();
                    conn.execute(
                        "INSERT INTO messages (id, queue_name, body, created_at, attributes, status, receive_count) VALUES (?1, ?2, ?3, ?4, ?5, 'active', 0)",
                        [
                            &new_message_id,
                            &source_queue,
                            &body,
                            &now,
                            &attributes.unwrap_or_else(|| "".to_string()),
                        ],
                    )?;

                    // Remove from DLQ
                    conn.execute(
                        "DELETE FROM dead_letter_messages WHERE id = ?1",
                        [&message_id]
                    )?;

                    redriven_count += 1;
                }

                Ok(redriven_count)
            })
            .await
    }

    pub async fn purge_dlq(&self, dlq_name: &str) -> Result<u32> {
        let dlq_name = dlq_name.to_string();

        self.connection
            .call(move |conn| {
                let changes = conn.execute(
                    "DELETE FROM dead_letter_messages WHERE dlq_name = ?1",
                    [&dlq_name],
                )?;
                Ok(changes as u32)
            })
            .await
    }

    #[allow(dead_code)]
    pub async fn record_queue_metric(
        &self,
        _queue_name: &str,
        _metric: &QueueMetric,
    ) -> Result<()> {
        // Placeholder - do nothing for now
        Ok(())
    }

    // Enhanced send_message with DelaySeconds and FIFO support

    // Enhanced send_message with DelaySeconds, FIFO, and Message Groups support
    pub async fn send_message_with_delay_and_group(
        &self,
        params: SendMessageParams<'_>,
    ) -> Result<()> {
        // Check if this is a FIFO queue and get configuration
        let queue_config = self.get_queue_config(params.queue_name).await?;
        let queue_name = params.queue_name.to_string();
        let message_id = params.message_id.to_string();
        let body = params.body.to_string();
        let created_at = Utc::now().to_rfc3339();
        let attributes = params.attributes.map(|s| s.to_string());
        let deduplication_id = params.deduplication_id.map(|s| s.to_string());
        let delay_until = params.delay_until.map(|s| s.to_string());
        let message_group_id = params.message_group_id.map(|s| s.to_string());

        let is_fifo = queue_config.as_ref().map(|c| c.is_fifo).unwrap_or(false);

        // For FIFO queues, ensure MessageGroupId is provided
        let message_group_id = if is_fifo && message_group_id.is_none() {
            // Use a default message group ID if none provided for backwards compatibility
            Some("default".to_string())
        } else {
            message_group_id
        };

        // For FIFO queues, handle content-based deduplication if enabled
        let effective_dedup_id = if is_fifo {
            match (deduplication_id.clone(), queue_config.as_ref()) {
                (Some(id), _) => Some(id), // Explicit deduplication ID provided
                (None, Some(config)) if config.content_based_deduplication => {
                    // Generate SHA-256 hash of message body for content-based deduplication
                    Some(format!("{:x}", md5::compute(body.as_bytes()))) // Using MD5 for simplicity
                }
                _ => None,
            }
        } else {
            deduplication_id.clone()
        };

        // Check for duplicate deduplication_id within the last 5 minutes
        if let Some(ref dedup_id) = effective_dedup_id {
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
                // Generate sequence number for FIFO queues
                let sequence_number = if is_fifo {
                    // Get the next sequence number for this queue
                    let mut stmt = conn.prepare(
                        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM messages WHERE queue_name = ?1"
                    )?;
                    let seq_num: i64 = stmt.query_row([&queue_name], |row| row.get(0))?;
                    Some(seq_num)
                } else {
                    None
                };

                conn.execute(
                    "INSERT INTO messages (id, queue_name, body, created_at, attributes, deduplication_id, delay_until, sequence_number, message_group_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    [
                        &Some(&message_id),
                        &Some(&queue_name),
                        &Some(&body),
                        &Some(&created_at),
                        &attributes.as_ref(),
                        &effective_dedup_id.as_ref(),
                        &delay_until.as_ref(),
                        &sequence_number.map(|n| n.to_string()).as_ref(),
                        &message_group_id.as_ref()
                    ],
                )?;
                Ok(())
            })
            .await
    }

    // SetQueueAttributes support
    pub async fn set_queue_attributes(
        &self,
        queue_name: &str,
        attributes: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let queue_name = queue_name.to_string();

        // Parse common SQS attributes
        let visibility_timeout = attributes
            .get("VisibilityTimeout")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(30);
        let message_retention_period = attributes
            .get("MessageRetentionPeriod")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(1209600);
        let delay_seconds = attributes
            .get("DelaySeconds")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let receive_message_wait_time = attributes
            .get("ReceiveMessageWaitTimeSeconds")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);

        // Parse RedrivePolicy JSON
        let (max_receive_count, dead_letter_target_arn) =
            if let Some(redrive_policy) = attributes.get("RedrivePolicy") {
                // Parse JSON: {"deadLetterTargetArn":"arn:aws:sqs:region:account:queue-name","maxReceiveCount":3}
                if let Ok(policy) = serde_json::from_str::<serde_json::Value>(redrive_policy) {
                    let max_count = policy
                        .get("maxReceiveCount")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                    let dlq_arn = policy
                        .get("deadLetterTargetArn")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    (max_count, dlq_arn)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        self.connection
            .call(move |conn| {
                conn.execute(
                    r#"
                    INSERT OR REPLACE INTO queue_config
                    (name, visibility_timeout_seconds, message_retention_period_seconds, delay_seconds,
                     receive_message_wait_time_seconds, max_receive_count, dead_letter_target_arn)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                    rusqlite::params![
                        queue_name,
                        visibility_timeout,
                        message_retention_period,
                        delay_seconds,
                        receive_message_wait_time,
                        max_receive_count,
                        dead_letter_target_arn
                    ],
                )?;
                Ok(())
            })
            .await
    }

    // Batch operations for Phase 2
    pub async fn send_messages_batch(
        &self,
        messages: Vec<DelayedMessageTuple>, // (queue_name, message_id, body, attributes, deduplication_id, delay_until)
    ) -> Result<Vec<std::result::Result<(), String>>> {
        let created_at = Utc::now().to_rfc3339();
        let mut results = Vec::new();

        self.connection
            .call(move |conn| {
                let tx = conn.unchecked_transaction()?;

                for (queue_name, message_id, body, attributes, deduplication_id, delay_until) in messages {
                    let result = (|| {
                        // Check for duplicate deduplication_id within the last 5 minutes if provided
                        if let Some(ref dedup_id) = deduplication_id {
                            let five_minutes_ago = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
                            let mut stmt = tx.prepare_cached(
                                "SELECT COUNT(*) FROM messages WHERE queue_name = ?1 AND deduplication_id = ?2 AND created_at > ?3"
                            )?;
                            let count: i64 = stmt.query_row([&queue_name, dedup_id, &five_minutes_ago], |row| {
                                row.get(0)
                            })?;

                            if count > 0 {
                                return Ok(()); // Silently ignore duplicate
                            }
                        }

                        tx.execute(
                            "INSERT INTO messages (id, queue_name, body, created_at, attributes, deduplication_id, delay_until) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            [
                                &Some(message_id.clone()),
                                &Some(queue_name.clone()),
                                &Some(body.clone()),
                                &Some(created_at.clone()),
                                &attributes,
                                &deduplication_id,
                                &delay_until
                            ],
                        )?;
                        Ok(())
                    })();

                    results.push(result.map_err(|e: rusqlite::Error| e.to_string()));
                }

                tx.commit()?;
                Ok(results)
            })
            .await
    }

    pub async fn delete_messages_batch(
        &self,
        message_ids: Vec<String>,
    ) -> Result<Vec<std::result::Result<bool, String>>> {
        let deleted_at = Utc::now().to_rfc3339();
        let mut results = Vec::new();

        self.connection
            .call(move |conn| {
                let tx = conn.unchecked_transaction()?;

                for message_id in message_ids {
                    let result = (|| {
                        let changes = tx.execute(
                            "UPDATE messages SET status = 'deleted', deleted_at = ?2 WHERE id = ?1",
                            [&message_id, &deleted_at],
                        )?;
                        Ok(changes > 0)
                    })();

                    results.push(result.map_err(|e: rusqlite::Error| e.to_string()));
                }

                tx.commit()?;
                Ok(results)
            })
            .await
    }

    pub async fn receive_messages_batch(
        &self,
        queue_name: &str,
        max_messages: u32,
    ) -> Result<Vec<(String, String, String, Option<String>)>> {
        let queue_name = queue_name.to_string();
        let processed_at = Utc::now().to_rfc3339();
        let max_messages = max_messages.min(10) as i64; // AWS SQS limit

        self.connection
            .call(move |conn| {
                let tx = conn.unchecked_transaction()?;

                let mut stmt = tx.prepare(
                    r#"
                    SELECT id, body, created_at, attributes
                    FROM messages
                    WHERE queue_name = ?1
                    AND status = 'active'
                    AND (visibility_timeout IS NULL OR visibility_timeout < datetime('now'))
                    AND (delay_until IS NULL OR delay_until < datetime('now'))
                    ORDER BY created_at ASC
                    LIMIT ?2
                    "#,
                )?;

                let rows = stmt.query_map([&queue_name, &max_messages.to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })?;

                let mut messages = Vec::new();
                for row in rows {
                    let (id, body, created_at, attributes) = row?;

                    // Set visibility timeout (30 seconds from now) and mark as processing
                    let timeout = (Utc::now() + chrono::Duration::seconds(30)).to_rfc3339();
                    tx.execute(
                        "UPDATE messages SET visibility_timeout = ?1, receive_count = receive_count + 1, status = 'processing', processed_at = ?3 WHERE id = ?2",
                        [&timeout, &id, &processed_at],
                    )?;

                    messages.push((id, body, created_at, attributes));
                }

                drop(stmt); // Explicitly drop the statement before committing
                tx.commit()?;
                Ok(messages)
            })
            .await
    }

    pub async fn cleanup_expired_messages(
        &self,
        retention_config: &crate::config::RetentionConfig,
    ) -> Result<u32> {
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
            }
            crate::config::RetentionMode::Delete => {
                // In Delete mode, actually delete messages older than the configured retention period
                let retention_days = retention_config.delete_after_days.unwrap_or(14);
                let retention_seconds = (retention_days as i64) * 24 * 3600;
                let cutoff_time = Utc::now() - chrono::Duration::seconds(retention_seconds);
                let cutoff_str = cutoff_time.to_rfc3339();

                self.connection
                    .call(move |conn| {
                        let mut stmt =
                            conn.prepare("DELETE FROM messages WHERE created_at < ?1")?;
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
#[allow(dead_code)]
pub struct QueueMetric {
    pub messages_sent: u32,
    pub messages_received: u32,
    pub messages_deleted: u32,
    pub processing_time_ms: u32,
}
