use std::sync::Arc;
use anyhow::Result;
use alloy::{
    network::EthereumWallet,
    primitives::{address, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
};
use rustyarb::executors::univ3::{UniV3Executor, UniV3SwapAction};
use rustyarb::types::Executor;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let private_key = std::env::var("PRIVATE_KEY")?;
    let rpc_url = std::env::var("RPC_URL")?;

    let signer: PrivateKeySigner = private_key.parse()?;
    let wallet = EthereumWallet::from(signer);

    let provider = Arc::new(
        ProviderBuilder::new()
            .wallet(wallet)
            .on_builtin(&rpc_url)
            .await?
    );

    let router_address = address!("0x6D99e7f6747AF2cDbB5164b6DD50e40D4fDe1e77");
    let usdc = address!("0xb88339cb7199b77e23db6e890353e22632ba630f");
    let whype = address!("0x5555555555555555555555555555555555555555");

    let executor = UniV3Executor::new(provider, &private_key, router_address)?;

    // Example: Swap 10 USDC for WHYPE
    let swap = UniV3SwapAction {
        token_in: usdc,
        token_out: whype,
        fee: 3000,
        amount_in: U256::from(1_000_000), // 10 USDC (6 decimals)
        amount_out_min: U256::from(0),
    };

    executor.execute(swap).await?;
    
    Ok(())
}
