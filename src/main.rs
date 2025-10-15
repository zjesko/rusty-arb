use std::sync::Arc;

use anyhow::Result;
use alloy::{
    providers::ProviderBuilder,
    transports::ws::WsConnect,
};
use rustyarb::{
    collectors::{
        uniswapv3::UniV3Collector,
        hyperliquid::HyperliquidCollector,
    },
    engine::Engine,
    strategies::hype_usdc_cross_arbitrage::{HypeUsdcCrossArbitrage, Event, Action},
    types::CollectorMap,
};
use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing
    let filter = filter::Targets::new()
        .with_target("rustyarb", Level::INFO);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let rpc_endpoint = "wss://hyperliquid-mainnet.g.alchemy.com/v2/";
    let ws = WsConnect::new(rpc_endpoint);
    let provider = Arc::new(ProviderBuilder::new().connect_ws(ws).await?);
    
    let mut engine: Engine<Event, Action> = Engine::default();

    let hyperswap_collector = Box::new(UniV3Collector::new(
        provider.clone(),
        alloy::primitives::address!("0xe712d505572b3f84c1b4deb99e1beab9dd0e23c9"),
    ));
    engine.add_collector(Box::new(CollectorMap::new(
        hyperswap_collector,
        |pool_state| Event::PoolUpdate(pool_state),
    )));

    let hyperliquid_collector = Box::new(HyperliquidCollector::new("@107".to_string()));
    engine.add_collector(Box::new(CollectorMap::new(
        hyperliquid_collector,
        |bbo| Event::HyperliquidBbo(bbo),
    )));

    engine.add_strategy(Box::new(HypeUsdcCrossArbitrage::new()));

    info!("Starting engine...");

    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            info!("Task completed: {:?}", res);
        }
    }

    Ok(())
}
