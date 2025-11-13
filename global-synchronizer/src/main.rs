use clap::{Arg, Command};
use global_synchronizer::{GlobalSynchronizer, config::GlobalSyncConfig, api::create_router, consensus_example};
use std::sync::Arc;
use axum::Router;
use tracing::{info, error};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let matches = Command::new("global-synchronizer")
        .version("0.1.0")
        .author("GARP Team")
        .about("Global Synchronizer for cross-domain atomic settlement with BFT consensus")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config/global-sync.toml")
        )
        .arg(
            Arg::new("node-id")
                .short('n')
                .long("node-id")
                .value_name("ID")
                .help("Global synchronizer node ID")
                .required(true)
        )
        .arg(
            Arg::new("cluster-peers")
                .short('p')
                .long("peers")
                .value_name("PEERS")
                .help("Comma-separated list of cluster peer addresses")
                .default_value("localhost:7000,localhost:7001,localhost:7002")
        )
        .arg(
            Arg::new("kafka-brokers")
                .short('k')
                .long("kafka-brokers")
                .value_name("BROKERS")
                .help("Comma-separated list of Kafka broker addresses")
                .default_value("localhost:9092")
        )
        .arg(
            Arg::new("database-url")
                .short('d')
                .long("database-url")
                .value_name("URL")
                .help("Database connection URL")
                .default_value("postgresql://garp:garp@localhost:5432/global_sync")
        )
        .arg(
            Arg::new("port")
                .long("port")
                .value_name("PORT")
                .help("API server port")
                .default_value("8000")
        )
        .arg(
            Arg::new("consensus-port")
                .long("consensus-port")
                .value_name("PORT")
                .help("Consensus protocol port")
                .default_value("7000")
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .value_name("LEVEL")
                .help("Log level (trace, debug, info, warn, error)")
                .default_value("info")
        )
        .arg(
            Arg::new("enable-metrics")
                .long("enable-metrics")
                .help("Enable Prometheus metrics endpoint")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    // Initialize tracing
    let log_level = matches.get_one::<String>("log-level").unwrap();
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Global Synchronizer v0.1.0");

    // Load configuration
    let config_path = PathBuf::from(matches.get_one::<String>("config").unwrap());
    let mut config = if config_path.exists() {
        GlobalSyncConfig::load(&config_path)?
    } else {
        info!("Configuration file not found, using default configuration");
        GlobalSyncConfig::default()
    };

    // Override configuration with command line arguments
    config.node.node_id = matches.get_one::<String>("node-id").unwrap().clone();
    
    if let Some(peers) = matches.get_one::<String>("cluster-peers") {
        config.consensus.cluster_peers = peers
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
    }
    
    if let Some(brokers) = matches.get_one::<String>("kafka-brokers") {
        config.kafka.bootstrap_servers = brokers
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
    }
    
    config.database.url = matches.get_one::<String>("database-url").unwrap().clone();
    config.api.port = matches.get_one::<String>("port").unwrap().parse()?;
    config.consensus.port = matches.get_one::<String>("consensus-port").unwrap().parse()?;
    config.monitoring.enable_metrics = matches.get_flag("enable-metrics");

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    info!("Configuration loaded successfully");
    info!("Node ID: {}", config.node.node_id);
    info!("Cluster peers: {:?}", config.consensus.cluster_peers);
    info!("API port: {}", config.api.port);
    info!("Consensus port: {}", config.consensus.port);
    
    // Demonstrate the new consensus system
    info!("Demonstrating new consensus system...");
    tokio::spawn(async {
        consensus_example::consensus_manager_example().await;
        consensus_example::consensus_engine_examples().await;
        consensus_example::reputation_scoring_example();
    });

    // Create global synchronizer
    let global_sync = match GlobalSynchronizer::new(config).await {
        Ok(sync) => sync,
        Err(e) => {
            error!("Failed to create Global Synchronizer: {}", e);
            std::process::exit(1);
        }
    };
    let sync_arc = Arc::new(global_sync);

    // Set up graceful shutdown
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Received shutdown signal");
    };

    // Start API server
    let api_port = sync_arc.api_port();
    let api_sync = sync_arc.clone();
    let api_handle = tokio::spawn(async move {
        let app: Router = create_router(api_sync);
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], api_port));
        let listener = tokio::net::TcpListener::bind(addr).await.expect("bind failed");
        axum::serve(listener, app).await.expect("server error");
    });

    // Start the global synchronizer
    tokio::select! {
        result = sync_arc.start() => {
            match result {
                Ok(_) => info!("Global Synchronizer stopped gracefully"),
                Err(e) => {
                    error!("Global Synchronizer error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        _ = shutdown_signal => {
            info!("Initiating graceful shutdown...");
            if let Err(e) = sync_arc.stop().await {
                error!("Error during shutdown: {}", e);
                std::process::exit(1);
            }
            info!("Global Synchronizer stopped gracefully");
        }
    }

    // Ensure API server task ends
    let _ = api_handle.abort();

    Ok(())
}