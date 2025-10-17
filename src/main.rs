use std::sync::Arc;

use anyhow::Result;
use alloy::{
    network::EthereumWallet,
    primitives::Address,
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    transports::ws::WsConnect,
};
use rustyarb::{
    collectors::{
        uniswapv3::UniV3Collector,
        hyperliquid::HyperliquidCollector,
    },
    config::Config,
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

    // Load environment variables
    dotenv::dotenv().ok();
    
    // Load configuration
    let config = Config::load("config.toml")?;
    info!("✓ Loaded config with {} strategies", config.strategies.len());
    
    // Get private key from env
    let private_key = std::env::var("PRIVATE_KEY")?;
    let signer: PrivateKeySigner = private_key.parse()?;
    let wallet = EthereumWallet::from(signer);
    
    // Connect to network
    let ws = WsConnect::new(&config.rpc_url_ws);
    let provider = Arc::new(
        ProviderBuilder::new()
            .wallet(wallet)
            .connect_ws(ws)
            .await?
    );
    
    // Create engine
    let mut engine: Engine<Event, Action> = Engine::default();
    
    // Process each enabled strategy
    let enabled_strategies: Vec<_> = config.strategies.iter()
        .filter(|s| s.enabled)
        .collect();
    
    if enabled_strategies.is_empty() {
        anyhow::bail!("No enabled strategies found in config");
    }
    
    let num_strategies = enabled_strategies.len();
    info!("🚀 Starting {} enabled strategies", num_strategies);
    
    for strategy_config in enabled_strategies {
        info!("  • {}", strategy_config.name);
        
        // Parse addresses
        let pool_address: Address = strategy_config.pool_address.parse()?;
        let router_address: Address = strategy_config.router_address.parse()?;
        
        // Add DEX collector (UniswapV3)
        let univ3_collector = Box::new(UniV3Collector::new(
            provider.clone(),
            pool_address,
        ));
        engine.add_collector(Box::new(CollectorMap::new(
            univ3_collector,
            |pool_state| Event::PoolUpdate(pool_state),
        )));
        
        // Add CEX collector (Hyperliquid)
        let hl_collector = Box::new(HyperliquidCollector::new(
            strategy_config.hyperliquid_coin.clone()
        ));
        engine.add_collector(Box::new(CollectorMap::new(
            hl_collector,
            |bbo| Event::HyperliquidBbo(bbo),
        )));
        
        // Add strategy
        let strategy = Box::new(HypeUsdcCrossArbitrage::from_config(strategy_config)?);
        engine.add_strategy(strategy);
        
        // Create per-strategy execution manager (1 execution at a time per strategy)
        let exec_manager = Arc::new(ExecutionManager::new(1));
        
        // Add executors
        let arb_executor = ArbitrageExecutor::new(
            UniV3Executor::new(provider.clone(), &private_key, router_address)?,
            HyperliquidExecutor::new(private_key.clone())?,
            exec_manager,
            config.cooldown_secs,
        );
        engine.add_executor(Box::new(arb_executor));
    }
    
    info!("🤖 RustyArb live | Min profit: {}bps | Strategies: {}",
        config.strategies.first().unwrap().min_profit_bps,
        num_strategies
    );
    
    // Run engine
    if let Ok(mut set) = engine.run().await {
        while set.join_next().await.is_some() {}
    }
    
    Ok(())
}
