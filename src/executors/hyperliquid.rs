use anyhow::Result;
use async_trait::async_trait;
use alloy::signers::local::PrivateKeySigner;
use hyperliquid_rust_sdk::{
    BaseUrl, ExchangeClient, ExchangeResponseStatus, 
    ClientOrderRequest, ClientOrder, ClientLimit
};
use tracing::{info, error};

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

        let order = ClientOrderRequest {
            asset: action.coin.clone(),
            is_buy: action.is_buy,
            reduce_only: false,
            limit_px: action.limit_px,
            sz: action.size,
            cloid: None,
            order_type: ClientOrder::Limit(ClientLimit {
                tif: "Ioc".to_string(),
            }),
        };

        let response = client.order(order, None).await?;

        match response {
            ExchangeResponseStatus::Ok(response) => {
                info!(
                    "Order response: {} {} {} | Status: {:?}",
                    if action.is_buy { "BUY" } else { "SELL" },
                    action.size,
                    action.coin,
                    response
                );
                
                if let Some(data) = response.data {
                    if !data.statuses.is_empty() {
                        info!("Order status: {:?}", data.statuses[0]);
                    }
                }
                
                Ok(())
            }
            ExchangeResponseStatus::Err(e) => {
                error!("Order failed: {:?}", e);
                anyhow::bail!("Order execution failed: {}", e)
            }
        }
    }
}


