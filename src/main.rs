mod database;
mod http_server;
mod message;
mod queue_service;
mod sqs_types;
mod ui;

use clap::{Parser, Subcommand};
use queue_service::QueueService;
use std::sync::Arc;

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
    let service = Arc::new(QueueService::new("qlite.db").await?);

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
            println!("Starting QLite SQS-compatible server on port {}", port);
            println!("Base URL: {}", base_url);
            
            let app = http_server::create_router(service, base_url, enable_ui);
            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
            
            println!("Server running at http://0.0.0.0:{}", port);
            if enable_ui {
                println!("Web UI available at http://localhost:{}/ui", port);
            }
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
