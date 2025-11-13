use clap::Parser;
use garp_common::{ParticipantConfig, GarpResult};
use garp_participant_node::{ParticipantNode, Config};
use tracing::{info, error};
use std::sync::Arc;
use garp_participant_node::api::ApiServer;
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "garp-participant-node")]
#[command(about = "GARP Blockchain Participant Node")]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
    
    #[arg(short, long)]
    participant_id: Option<String>,
    
    #[arg(short, long)]
    database_url: Option<String>,
    
    #[arg(long, default_value = "8080")]
    api_port: u16,
}

#[tokio::main]
async fn main() -> GarpResult<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("garp_participant_node=info,garp_common=info")
        .init();

    let args = Args::parse();

    // Load configuration
    let config = Config::load(&args.config, args.participant_id, args.database_url, args.api_port)?;
    let api_config = config.api.clone();
    
    info!("Starting GARP Participant Node: {}", config.participant_config.participant_id.0);
    info!("API listening on port: {}", config.participant_config.api_port);
    info!("Database URL: {}", config.participant_config.database_url);

    // Create and start the participant node
    let mut node = ParticipantNode::new(config).await?;

    // Start the node
    if let Err(e) = node.start().await {
        error!("Failed to start participant node: {:?}", e);
        return Err(e);
    }

    // Move node into Arc for API server and shutdown handling
    let node = Arc::new(node);

    // Start API server concurrently
    {
        let node_for_api = node.clone();
        tokio::spawn(async move {
            let api_server = ApiServer::new(node_for_api, api_config);
            if let Err(e) = api_server.start().await {
                error!("API server error: {:?}", e);
            }
        });
    }

    // Keep the node running
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
    info!("Shutting down participant node...");

    node.shutdown().await?;
    info!("Participant node shut down successfully");

    Ok(())
}