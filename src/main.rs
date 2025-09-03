mod config;
mod database;
mod http_server;
mod message;
mod queue_service;
mod retention;
mod sqs_types;
mod ui;

use clap::{Parser, Subcommand};
use config::Config;
use queue_service::QueueService;
use retention::BackgroundServices;
use std::sync::Arc;
use tracing::info;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    CreateQueue {
        #[arg(short, long)]
        name: String,
    },
    Send {
        #[arg(short, long)]
        queue: String,
        #[arg(short, long)]
        message: String,
    },
    Receive {
        #[arg(short, long)]
        queue: String,
    },
    Delete {
        #[arg(short, long)]
        receipt_handle: String,
    },
    Server {
        #[arg(short, long, default_value = "3000")]
        port: u16,
        #[arg(long, default_value = "http://localhost:3000")]
        base_url: String,
        #[arg(long, default_value = "false")]
        enable_ui: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    // Load configuration with environment overrides and defaults
    let config = Config::load_with_overrides().unwrap_or_else(|e| {
        println!("Warning: Failed to load config: {}. Using defaults.", e);
        Config::default()
    });
    
    let service = Arc::new(QueueService::new(&config.database.path).await?);

    match cli.command {
        Commands::CreateQueue { name } => {
            service.create_queue(&name).await?;
            println!("Queue '{}' created successfully", name);
        }
        Commands::Send { queue, message } => {
            let message_id = service.send_message(&queue, &message, None, None).await?;
            println!("Message sent with ID: {}", message_id);
        }
        Commands::Receive { queue } => {
            if let Some(msg) = service.receive_message(&queue).await? {
                println!("Received message:");
                println!("  ID: {}", msg.id);
                println!("  Body: {}", msg.body);
                println!("  Receipt Handle: {}", msg.receipt_handle);
                if let Some(attrs) = msg.attributes {
                    println!("  Attributes: {:?}", attrs);
                }
            } else {
                println!("No messages available in queue '{}'", queue);
            }
        }
        Commands::Delete { receipt_handle } => {
            if service.delete_message(&receipt_handle).await? {
                println!("Message deleted successfully");
            } else {
                println!("Message not found or already deleted");
            }
        }
        Commands::Server { port, base_url, enable_ui } => {
            // Override config with CLI arguments
            let mut server_config = config.clone();
            server_config.server.port = port;
            server_config.server.base_url = Some(base_url.clone());
            server_config.server.enable_ui = enable_ui;
            
            println!("Starting QLite SQS-compatible server on port {}", port);
            println!("Base URL: {}", base_url);
            
            // Start background services
            let mut background_services = BackgroundServices::new();
            background_services.start_retention_cleanup(Arc::clone(&service), server_config.clone()).await?;
            info!("Background retention cleanup service started");
            
            // Setup graceful shutdown
            let shutdown_signal = async {
                tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
                info!("Received shutdown signal, initiating graceful shutdown");
            };
            
            let app = http_server::create_router(service, base_url, enable_ui);
            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
            
            println!("Server running at http://0.0.0.0:{}", port);
            if enable_ui {
                println!("Web UI available at http://localhost:{}/ui", port);
            }
            println!("Press Ctrl+C to shutdown gracefully");
            
            // Run server with graceful shutdown
            let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal);
            
            match server.await {
                Ok(_) => info!("Server shutdown completed successfully"),
                Err(e) => println!("Server error: {}", e),
            }
            
            // Cleanup background services
            info!("Cleaning up background services...");
            // Background services will be dropped and cleaned up automatically
        }
    }

    Ok(())
}
