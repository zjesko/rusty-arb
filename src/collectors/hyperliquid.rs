use anyhow::Result;
use async_trait::async_trait;
use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Message, Subscription};
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};

use crate::types::{Collector, CollectorStream};

#[derive(Debug, Clone)]
pub struct HyperliquidBbo {
    pub coin: String,
    pub levels: Vec<Option<hyperliquid_rust_sdk::BookLevel>>,
    pub time: u64,
}

pub struct HyperliquidCollector {
    coin: String,
}

impl HyperliquidCollector {
    pub fn new(coin: String) -> Self {
        Self { coin }
    }
}

#[async_trait]
impl Collector<HyperliquidBbo> for HyperliquidCollector {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, HyperliquidBbo>> {
        let mut info_client = InfoClient::new(None, Some(BaseUrl::Mainnet))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create InfoClient: {:?}", e))?;

        let (sender, receiver) = unbounded_channel();
        
        let _subscription_id = info_client
            .subscribe(Subscription::Bbo { coin: self.coin.clone() }, sender)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to subscribe to BBO: {:?}", e))?;

        tokio::spawn(async move {
            let _client = info_client;
            std::future::pending::<()>().await;
        });

        let stream = UnboundedReceiverStream::new(receiver).filter_map(|msg| {
            match msg {
                Message::Bbo(bbo) => {
                    Some(HyperliquidBbo {
                        coin: bbo.data.coin,
                        levels: bbo.data.bbo,
                        time: bbo.data.time,
                    })
                }
                _ => None,
            }
        });

        Ok(Box::pin(stream))
    }
}