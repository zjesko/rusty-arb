use anyhow::Result;
use rustyarb::executors::hyperliquid::{HyperliquidExecutor, HyperliquidOrderAction};
use rustyarb::types::Executor;
use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

#[tokio::main]
async fn main() -> Result<()> {
    let filter = filter::Targets::new().with_default(Level::INFO);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let private_key = std::env::var("PRIVATE_KEY")
        .expect("PRIVATE_KEY environment variable not set");

    info!("Initializing Hyperliquid executor...");
    let executor = HyperliquidExecutor::new(private_key)?;

    let test_action = HyperliquidOrderAction {
        coin: "HYPE/USDC".to_string(),
        is_buy: false,
        size: 1.0,
        limit_px: 32.0, // ~$40 + 20% = $48 (within 95% tolerance)
    };

    info!("Placing test order: BUY {} {} @ ${:.2}", test_action.size, test_action.coin, test_action.limit_px);
    executor.execute(test_action).await?;

    info!("Test completed successfully!");
    Ok(())
}
