use chrono::Utc;
use tokio_rusqlite::{Connection, OptionalExtension, Result};

pub struct Database {
    connection: Connection,
}

impl Database {
    pub async fn new(db_path: &str) -> Result<Self> {
        let connection = Connection::open(db_path).await?;
        
        let db = Database { connection };
        db.init_schema().await?;
        
        Ok(db)
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
                        deduplication_id TEXT
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
        let _visibility_timeout = Utc::now().to_rfc3339();

        self.connection
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT id, body, created_at, attributes 
                    FROM messages 
                    WHERE queue_name = ?1 
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
                    
                    // Set visibility timeout (30 seconds from now)
                    let timeout = (Utc::now() + chrono::Duration::seconds(30)).to_rfc3339();
                    conn.execute(
                        "UPDATE messages SET visibility_timeout = ?1, receive_count = receive_count + 1 WHERE id = ?2",
                        [&timeout, &id],
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

        self.connection
            .call(move |conn| {
                let changes = conn.execute(
                    "DELETE FROM messages WHERE id = ?1",
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
                    "SELECT id, body, created_at, visibility_timeout, receive_count, attributes, deduplication_id FROM messages WHERE queue_name = ?1 ORDER BY created_at ASC"
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
                
                // Get message counts
                let mut stmt = conn.prepare(
                    "SELECT COUNT(*) FROM messages WHERE queue_name = ?1"
                )?;
                let total_messages: i64 = stmt.query_row([&queue_name], |row| row.get(0))?;
                
                let mut stmt = conn.prepare(
                    "SELECT COUNT(*) FROM messages WHERE queue_name = ?1 AND (visibility_timeout IS NULL OR visibility_timeout < datetime('now'))"
                )?;
                let visible_messages: i64 = stmt.query_row([&queue_name], |row| row.get(0))?;
                
                let in_flight_messages = total_messages - visible_messages;
                
                Ok(Some(QueueAttributes {
                    approximate_number_of_messages: visible_messages as u32,
                    approximate_number_of_messages_not_visible: in_flight_messages as u32,
                    created_timestamp: queue_exists.unwrap(),
                }))
            })
            .await
    }
}

#[derive(Debug, Clone)]
pub struct QueueAttributes {
    pub approximate_number_of_messages: u32,
    pub approximate_number_of_messages_not_visible: u32,
    pub created_timestamp: String,
}