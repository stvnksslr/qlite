use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub queues: QueueDefaults,
    pub metrics: MetricsConfig,
    pub retention: RetentionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub enable_ui: bool,
    pub base_url: Option<String>,
    pub max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
    pub connection_pool_size: usize,
    pub busy_timeout_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueDefaults {
    pub visibility_timeout_seconds: u32,
    pub message_retention_seconds: u32,
    pub max_receive_count: u32,
    pub receive_message_wait_time_seconds: u32,
    pub fifo_throughput_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub collection_interval_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    pub cleanup_interval_seconds: u32,
    pub batch_size: u32,
    pub mode: RetentionMode,
    pub delete_after_days: Option<u32>, // Only used in Delete mode
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum RetentionMode {
    /// Keep messages forever, mark as hidden when processed (default)
    KeepForever,
    /// Actually delete messages after retention period
    Delete,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                port: 3000,
                host: "0.0.0.0".to_string(),
                enable_ui: false,
                base_url: None,
                max_connections: 1000,
            },
            database: DatabaseConfig {
                path: "qlite.db".to_string(),
                connection_pool_size: 10,
                busy_timeout_ms: 5000,
            },
            queues: QueueDefaults {
                visibility_timeout_seconds: 30,
                message_retention_seconds: 1209600, // 14 days
                max_receive_count: 10,
                receive_message_wait_time_seconds: 0,
                fifo_throughput_limit: 300,
            },
            metrics: MetricsConfig {
                enabled: true,
                endpoint: "/metrics".to_string(),
                collection_interval_seconds: 60,
            },
            retention: RetentionConfig {
                cleanup_interval_seconds: 3600, // 1 hour
                batch_size: 1000,
                mode: RetentionMode::KeepForever, // Default: keep messages forever
                delete_after_days: Some(14),      // Only used in Delete mode
            },
        }
    }
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;

        let config: Config =
            toml::from_str(&contents).map_err(|e| ConfigError::Parse(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    pub fn load_with_overrides() -> Result<Self, ConfigError> {
        let mut config = if Path::new("qlite.toml").exists() {
            Self::load_from_file("qlite.toml")?
        } else {
            Self::default()
        };

        // Apply environment variable overrides
        config.apply_env_overrides();
        config.validate()?;

        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(port) = std::env::var("QLITE_PORT")
            && let Ok(port_num) = port.parse::<u16>()
        {
            self.server.port = port_num;
        }

        if let Ok(host) = std::env::var("QLITE_HOST") {
            self.server.host = host;
        }

        if let Ok(db_path) = std::env::var("QLITE_DB_PATH") {
            self.database.path = db_path;
        }

        if let Ok(enable_ui) = std::env::var("QLITE_ENABLE_UI") {
            self.server.enable_ui = enable_ui.to_lowercase() == "true";
        }

        if let Ok(base_url) = std::env::var("QLITE_BASE_URL") {
            self.server.base_url = Some(base_url);
        }

        if let Ok(metrics_enabled) = std::env::var("QLITE_METRICS_ENABLED") {
            self.metrics.enabled = metrics_enabled.to_lowercase() == "true";
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.server.port == 0 {
            return Err(ConfigError::Validation(
                "Server port cannot be 0".to_string(),
            ));
        }

        if self.server.host.is_empty() {
            return Err(ConfigError::Validation(
                "Server host cannot be empty".to_string(),
            ));
        }

        if self.database.path.is_empty() {
            return Err(ConfigError::Validation(
                "Database path cannot be empty".to_string(),
            ));
        }

        if self.queues.visibility_timeout_seconds == 0 {
            return Err(ConfigError::Validation(
                "Visibility timeout must be > 0".to_string(),
            ));
        }

        if self.queues.message_retention_seconds < 60 {
            return Err(ConfigError::Validation(
                "Message retention must be >= 60 seconds".to_string(),
            ));
        }

        if self.queues.message_retention_seconds > 1209600 {
            // 14 days
            return Err(ConfigError::Validation(
                "Message retention cannot exceed 14 days".to_string(),
            ));
        }

        if self.queues.max_receive_count == 0 {
            return Err(ConfigError::Validation(
                "Max receive count must be > 0".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    pub name: String,
    pub is_fifo: bool,
    pub content_based_deduplication: bool,
    pub visibility_timeout_seconds: u32,
    pub message_retention_period_seconds: u32,
    pub max_receive_count: Option<u32>,
    pub dead_letter_target_arn: Option<String>,
    pub delay_seconds: u32,
    pub receive_message_wait_time_seconds: u32,
}

// QueueType enum removed - using is_fifo boolean instead

impl Default for QueueConfig {
    fn default() -> Self {
        let defaults = Config::default().queues;
        Self {
            name: String::new(),
            is_fifo: false,
            content_based_deduplication: false,
            visibility_timeout_seconds: defaults.visibility_timeout_seconds,
            message_retention_period_seconds: defaults.message_retention_seconds,
            max_receive_count: Some(defaults.max_receive_count),
            dead_letter_target_arn: None,
            delay_seconds: 0,
            receive_message_wait_time_seconds: defaults.receive_message_wait_time_seconds,
        }
    }
}

impl QueueConfig {
    #[allow(dead_code)]
    pub fn new(name: String, is_fifo: bool) -> Self {
        Self {
            name,
            is_fifo,
            content_based_deduplication: is_fifo,
            ..Self::default()
        }
    }

    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.name.is_empty() {
            return Err(ConfigError::Validation(
                "Queue name cannot be empty".to_string(),
            ));
        }

        if self.is_fifo {
            if !self.name.ends_with(".fifo") {
                return Err(ConfigError::Validation(
                    "FIFO queue names must end with .fifo".to_string(),
                ));
            }
        } else if self.name.ends_with(".fifo") {
            return Err(ConfigError::Validation(
                "Standard queue names cannot end with .fifo".to_string(),
            ));
        }

        if self.visibility_timeout_seconds == 0 {
            return Err(ConfigError::Validation(
                "Visibility timeout must be > 0".to_string(),
            ));
        }

        if self.message_retention_period_seconds < 60 {
            return Err(ConfigError::Validation(
                "Message retention must be >= 60 seconds".to_string(),
            ));
        }

        if self.message_retention_period_seconds > 1209600 {
            // 14 days
            return Err(ConfigError::Validation(
                "Message retention cannot exceed 14 days".to_string(),
            ));
        }

        if let Some(max_count) = self.max_receive_count
            && max_count == 0
        {
            return Err(ConfigError::Validation(
                "Max receive count must be > 0".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    Validation(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(msg) => write!(f, "IO error: {}", msg),
            ConfigError::Parse(msg) => write!(f, "Parse error: {}", msg),
            ConfigError::Validation(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.database.path, "qlite.db");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_fifo_queue_validation() {
        let mut config = QueueConfig::new("test.fifo".to_string(), true);
        assert!(config.validate().is_ok());

        config.name = "test".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_standard_queue_validation() {
        let mut config = QueueConfig::new("test".to_string(), false);
        assert!(config.validate().is_ok());

        config.name = "test.fifo".to_string();
        assert!(config.validate().is_err());
    }
}
