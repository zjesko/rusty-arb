use std::sync::Arc;

use anyhow::Result;
use alloy::{
    network::EthereumWallet,
    primitives::address,
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    transports::ws::WsConnect,
};
use rustyarb::{
    collectors::{
        uniswapv3::UniV3Collector,
        hyperliquid::HyperliquidCollector,
    },
    engine::Engine,
    execution::ExecutionManager,
    executors::{
        arbitrage::ArbitrageExecutor,
        univ3::UniV3Executor,
        hyperliquid::HyperliquidExecutor,
    },
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

    dotenv::dotenv().ok();
    
    let ws_rpc_url = std::env::var("RPC_URL_WS")?;
    let private_key = std::env::var("PRIVATE_KEY")?;
    
    let signer: PrivateKeySigner = private_key.parse()?;
    let wallet = EthereumWallet::from(signer);
    
    let ws = WsConnect::new(ws_rpc_url);
    let provider = Arc::new(
        ProviderBuilder::new()
            .wallet(wallet)
            .connect_ws(ws)
            .await?
    );
    
    // Token addresses (Hyperliquid mainnet)
    let usdc_address = address!("0xb88339cb7199b77e23db6e890353e22632ba630f");
    let hype_address = address!("0x5555555555555555555555555555555555555555");
    let pool_address = address!("0xe712d505572b3f84c1b4deb99e1beab9dd0e23c9");
    
    let mut engine: Engine<Event, Action> = Engine::default();

    // Add collectors
    let hyperswap_collector = Box::new(UniV3Collector::new(
        provider.clone(),
        pool_address,
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

    let strategy = Box::new(HypeUsdcCrossArbitrage::new(
        20.0,       // order_size_usd
        2.0,        // hl_maker_fee_bps
        0.0001,     // dex_gas_fee_usd
        10.0,       // min_profit_bps
        usdc_address,
        hype_address,
        3000,       // dex_fee
    ));
    engine.add_strategy(strategy);

    // Setup executors
    let exec_manager = Arc::new(ExecutionManager::new(1));
    let router_address = address!("0x6D99e7f6747AF2cDbB5164b6DD50e40D4fDe1e77");
    let arb_executor = ArbitrageExecutor::new(
        UniV3Executor::new(provider.clone(), &private_key, router_address)?,
        HyperliquidExecutor::new(private_key)?,
        exec_manager,
    );
    engine.add_executor(Box::new(arb_executor));

    info!("🤖 RustyArb live | $20/trade | 10bps min");

    if let Ok(mut set) = engine.run().await {
        while set.join_next().await.is_some() {}
    }

    Ok(())
}
