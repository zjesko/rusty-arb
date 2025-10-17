use anyhow::Result;
use async_trait::async_trait;
use alloy::signers::local::PrivateKeySigner;
use hyperliquid_rust_sdk::{
    BaseUrl, ExchangeClient, ExchangeResponseStatus, InfoClient,
    ClientOrderRequest, ClientOrder, ClientLimit
};
use tracing::{error, info};

use crate::types::Executor;

#[derive(Debug, Clone)]
pub struct HyperliquidOrderAction {
    pub coin: String,
    pub is_buy: bool,
    pub size: f64,
    pub limit_px: f64,
}

pub struct HyperliquidExecutor {
    signer: PrivateKeySigner,
}

impl HyperliquidExecutor {
    pub fn new(private_key: String) -> Result<Self> {
        let signer = private_key.parse::<PrivateKeySigner>()?;
        Ok(Self { signer })
    }
}

#[async_trait]
impl Executor<HyperliquidOrderAction> for HyperliquidExecutor {
    async fn execute(&self, action: HyperliquidOrderAction) -> Result<()> {
        let client = ExchangeClient::new(
            None,
            self.signer.clone(),
            Some(BaseUrl::Mainnet),
            None,
            None,
        )
        .await?;

        // Get asset metadata for size decimals
        let info_client = InfoClient::new(None, Some(BaseUrl::Mainnet)).await?;
        let meta = info_client.meta().await?;
        
        let asset_info = meta.universe.iter()
            .find(|asset| {
                let pair = format!("{}/USDC", asset.name);
                pair == action.coin || asset.name == action.coin
            });
        
        let sz_decimals = asset_info.map(|info| info.sz_decimals as u32).unwrap_or(1);
        
        // Round size and price to HL requirements
        let tick_size = 0.001; // HYPE/USDC tick size
        let size_multiplier = 10_f64.powi(sz_decimals as i32);
        let rounded_size = (action.size * size_multiplier).round() / size_multiplier;
        let rounded_price = (action.limit_px / tick_size).round() * tick_size;
        
        let order_value = rounded_size * rounded_price;
        if order_value < 10.0 {
            anyhow::bail!("Order value ${:.2} below HL minimum", order_value);
        }

        let order = ClientOrderRequest {
            asset: action.coin.clone(),
            is_buy: action.is_buy,
            reduce_only: false,
            limit_px: rounded_price,
            sz: rounded_size,
            cloid: None,
            order_type: ClientOrder::Limit(ClientLimit {
                tif: "Ioc".to_string(),
            }),
        };

        let response = client.order(order, None).await?;

        match response {
            ExchangeResponseStatus::Ok(resp) => {
                // Log fill info if available
                if let Some(data) = &resp.data {
                    if let Some(status) = data.statuses.first() {
                        match status {
                            hyperliquid_rust_sdk::ExchangeDataStatus::Filled(_) => {
                                info!("HL: {:.1} @ ${:.3}", rounded_size, rounded_price);
                            }
                            _ => {
                                info!("HL: {:.1} @ ${:.3}", rounded_size, rounded_price);
                            }
                        }
                    }
                }
                Ok(())
            }
            ExchangeResponseStatus::Err(e) => {
                error!("HL: {:?}", e);
                anyhow::bail!("HL failed: {}", e)
            }
        }
    }
}


