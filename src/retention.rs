use crate::config::Config;
use crate::queue_service::QueueService;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, error};

pub struct RetentionCleanupService {
    scheduler: JobScheduler,
    queue_service: Arc<QueueService>,
    config: Config,
}

impl RetentionCleanupService {
    pub async fn new(queue_service: Arc<QueueService>, config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        let scheduler = JobScheduler::new().await?;
        
        Ok(Self {
            scheduler,
            queue_service,
            config,
        })
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let queue_service = Arc::clone(&self.queue_service);
        let cleanup_interval = self.config.retention.cleanup_interval_seconds;
        
        // Create a cron job that runs every cleanup_interval_seconds
        let cron_expression = if cleanup_interval < 60 {
            // For testing, run every minute
            "0 * * * * *".to_string()
        } else if cleanup_interval < 3600 {
            // Run every N minutes
            let minutes = cleanup_interval / 60;
            format!("0 */{} * * * *", minutes)
        } else {
            // Run every N hours
            let hours = cleanup_interval / 3600;
            format!("0 0 */{} * * *", hours)
        };

        let retention_config = self.config.retention.clone();
        let job = Job::new_async(&cron_expression, move |_uuid, _l| {
            let queue_service_clone = Arc::clone(&queue_service);
            let retention_config_clone = retention_config.clone();
            Box::pin(async move {
                Self::run_cleanup(queue_service_clone, retention_config_clone).await;
            })
        })?;

        self.scheduler.add(job).await?;
        self.scheduler.start().await?;
        
        info!("Retention cleanup service started with interval: {} seconds", cleanup_interval);
        Ok(())
    }

    async fn run_cleanup(queue_service: Arc<QueueService>, retention_config: crate::config::RetentionConfig) {
        info!("Starting message retention cleanup (mode: {:?})", retention_config.mode);
        
        match queue_service.cleanup_expired_messages(&retention_config).await {
            Ok(affected_count) => {
                if affected_count > 0 {
                    let action = match retention_config.mode {
                        crate::config::RetentionMode::KeepForever => "messages reset for retry",
                        crate::config::RetentionMode::Delete => "expired messages deleted",
                    };
                    info!("Cleanup completed: {} {}", affected_count, action);
                } else {
                    info!("Cleanup completed: no messages required processing");
                }
            },
            Err(e) => {
                error!("Failed to run retention cleanup: {}", e);
            }
        }
    }
}

// Background service for handling all periodic tasks
pub struct BackgroundServices {
    retention_service: Option<RetentionCleanupService>,
}

impl BackgroundServices {
    pub fn new() -> Self {
        Self {
            retention_service: None,
        }
    }

    pub async fn start_retention_cleanup(
        &mut self,
        queue_service: Arc<QueueService>,
        config: Config,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let service = RetentionCleanupService::new(queue_service, config).await?;
        service.start().await?;
        self.retention_service = Some(service);
        Ok(())
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_background_services_creation() {
        let services = BackgroundServices::new();
        assert!(services.retention_service.is_none());
    }
}