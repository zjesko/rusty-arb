use std::sync::Arc;

use anyhow::Result;
use alloy::{
    providers::ProviderBuilder,
    transports::ws::WsConnect,
};
use rustyarb::{
    collectors::uniswapv3::UniV3Collector,
    engine::Engine,
    types::{CollectorMap},
};
use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

#[derive(Debug, Clone)]
pub enum Event {
    PoolUpdate(Vec<alloy::primitives::Address>),
}

#[derive(Debug, Clone)]
pub enum Action {
    // Placeholder for future actions
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing
    let filter = filter::Targets::new()
        .with_target("rustyarb", Level::INFO);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let rpc_endpoint = "";
    info!("Connecting to WebSocket endpoint: {}", rpc_endpoint);
    
    let ws = WsConnect::new(rpc_endpoint);
    let provider = Arc::new(ProviderBuilder::new().connect_ws(ws).await?);
    
    // Set up engine
    let mut engine: Engine<Event, Action> = Engine::default();

    // Set up collector for Hyperswap WHYPE/USDC pool
    let hyperswap_collector = Box::new(UniV3Collector::new(
        provider.clone(),
        alloy::primitives::address!("0xe712d505572b3f84c1b4deb99e1beab9dd0e23c9"),
    ));
    let hyperswap_collector = CollectorMap::new(
        hyperswap_collector,
        |amms| Event::PoolUpdate(amms),
    );
    engine.add_collector(Box::new(hyperswap_collector));

    info!("Starting engine...");

    // Start engine
    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            info!("Task completed: {:?}", res);
        }
    }

    Ok(())
}
