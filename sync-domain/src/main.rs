use clap::Parser;
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sync_domain::{
    config::SyncDomainConfig,
    domain::SyncDomain,
};

#[derive(Parser)]
#[command(name = "sync-domain")]
#[command(about = "GARP Synchronization Domain Service")]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "sync-domain.toml")]
    config: String,

    /// Domain ID
    #[arg(short, long)]
    domain_id: Option<String>,

    /// Kafka bootstrap servers
    #[arg(long)]
    kafka_brokers: Option<String>,

    /// Database URL
    #[arg(long)]
    database_url: Option<String>,

    /// API port
    #[arg(short, long)]
    port: Option<u16>,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("sync_domain={},tower_http=debug", args.log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting GARP Synchronization Domain Service");

    // Load configuration
    let mut config = SyncDomainConfig::load(&args.config)?;

    // Override config with command line arguments
    if let Some(domain_id) = args.domain_id {
        config.domain_id = domain_id;
    }
    if let Some(kafka_brokers) = args.kafka_brokers {
        config.kafka.bootstrap_servers = kafka_brokers;
    }
    if let Some(database_url) = args.database_url {
        config.database.url = database_url;
    }
    if let Some(port) = args.port {
        config.api.port = port;
    }

    // Validate configuration
    config.validate()?;

    info!("Configuration loaded successfully");
    info!("Domain ID: {}", config.domain_id);
    info!("Kafka brokers: {}", config.kafka.bootstrap_servers);
    info!("API port: {}", config.api.port);

    // Create and start sync domain
    let sync_domain = Arc::new(SyncDomain::new(config).await?);
    
    // Setup graceful shutdown
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Shutdown signal received");
    };

    // Start the sync domain
    tokio::select! {
        result = sync_domain.start() => {
            if let Err(e) = result {
                error!("Sync domain error: {}", e);
                return Err(e.into());
            }
        }
        _ = shutdown_signal => {
            info!("Shutting down sync domain...");
            if let Err(e) = sync_domain.stop().await {
                error!("Error during shutdown: {}", e);
            }
        }
    }

    info!("Sync domain service stopped");
    Ok(())
}