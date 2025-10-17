use std::sync::Arc;
use anyhow::Result;
use alloy::{
    network::EthereumWallet,
    primitives::{address, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
};
use rustyarb::{
    execution::ExecutionManager,
    executors::{
        arbitrage::{ArbitrageExecutor, ArbitrageAction},
        univ3::{UniV3Executor, UniV3SwapAction},
        hyperliquid::{HyperliquidExecutor, HyperliquidOrderAction},
    },
    types::Executor,
};
use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

#[tokio::main]
async fn main() -> Result<()> {
    let filter = filter::Targets::new().with_target("rustyarb", Level::INFO);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    dotenv::dotenv().ok();
    
    let rpc_url = std::env::var("RPC_URL")?;
    let private_key = std::env::var("PRIVATE_KEY")?;

    let signer: PrivateKeySigner = private_key.parse()?;
    let wallet = EthereumWallet::from(signer);
    let provider = Arc::new(
        ProviderBuilder::new()
            .wallet(wallet)
            .connect(&rpc_url)
            .await?
    );

    let usdc = address!("0xb88339cb7199b77e23db6e890353e22632ba630f");
    let hype = address!("0x5555555555555555555555555555555555555555");
    let router_address = address!("0x6D99e7f6747AF2cDbB5164b6DD50e40D4fDe1e77");

    let exec_manager = Arc::new(ExecutionManager::new(1));
    let arb_executor = ArbitrageExecutor::new(
        UniV3Executor::new(provider.clone(), &private_key, router_address)?,
        HyperliquidExecutor::new(private_key.clone())?,
        exec_manager,
    );

    let test_scenarios = vec![
        ArbitrageAction {
            dex_swap: UniV3SwapAction {
                token_in: usdc,
                token_out: hype,
                fee: 3000,
                amount_in: U256::from(11_000_000),
                amount_out_min: U256::ZERO,
            },
            hl_order: HyperliquidOrderAction {
                coin: "HYPE/USDC".to_string(),
                is_buy: false,
                size: 0.3,
                limit_px: 20.0,
            },
            direction: "Buy DEX → Sell HL".to_string(),
        },
        ArbitrageAction {
            dex_swap: UniV3SwapAction {
                token_in: hype,
                token_out: usdc,
                fee: 3000,
                amount_in: U256::from(300_000_000_000_000_000u128),
                amount_out_min: U256::ZERO,
            },
            hl_order: HyperliquidOrderAction {
                coin: "HYPE/USDC".to_string(),
                is_buy: true,
                size: 0.3,
                limit_px: 40.0,
            },
            direction: "Buy HL → Sell DEX".to_string(),
        },
    ];

    info!("🧪 Testing both directions | $11 orders | 15s cooldown");

    for (i, test_action) in test_scenarios.into_iter().enumerate() {
        info!("Test {}: {}", i + 1, test_action.direction);
        
        let start = std::time::Instant::now();
        match arb_executor.execute(test_action).await {
            Ok(_) => {
                info!("✅ Test {} passed ({:.1}s)", i + 1, start.elapsed().as_secs_f32());
            }
            Err(e) => {
                info!("❌ Test {} failed: {}", i + 1, e);
            }
        }
    }

    info!("🎉 Tests complete");

    Ok(())
}

