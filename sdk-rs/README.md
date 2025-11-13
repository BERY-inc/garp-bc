# GARP Rust SDK

Async Rust client for GARP participant-node JSON-RPC.

Usage:

```rust
use garp_sdk::GarpClient;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let client = GarpClient::new("http://localhost:8080")?;
let slot = client.get_slot().await?;
let leader = client.get_slot_leader().await?;
let block = client.get_block_by_slot(slot).await?;
# Ok(())
# }
```