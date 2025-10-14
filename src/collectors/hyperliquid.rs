use anyhow::Result;
use async_trait::async_trait;
use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Message, Subscription};
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};

use crate::types::{Collector, CollectorStream};

/// Event emitted by the Hyperliquid collector containing BBO (Best Bid/Offer) data
#[derive(Debug, Clone)]
pub struct HyperliquidBbo {
    pub coin: String,
    pub levels: Vec<Option<hyperliquid_rust_sdk::BookLevel>>,
    pub time: u64,
}

/// Collector for Hyperliquid spot market BBO (Best Bid/Offer) data
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
        tracing::info!("Initializing HyperliquidCollector for coin: {}", self.coin);
        
        // Create info client (mainnet)
        let mut info_client = InfoClient::new(None, Some(BaseUrl::Mainnet))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create InfoClient: {:?}", e))?;

        let (sender, receiver) = unbounded_channel();
        
        // Subscribe to BBO feed
        let _subscription_id = info_client
            .subscribe(Subscription::Bbo { coin: self.coin.clone() }, sender)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to subscribe to BBO: {:?}", e))?;

        tracing::info!("Successfully subscribed to BBO for {}", self.coin);

        // Spawn a task to keep info_client alive
        // This prevents the WebSocket connection from being dropped
        tokio::spawn(async move {
            // Keep the info_client alive by holding it here
            // The task will run indefinitely
            let _client = info_client;
            std::future::pending::<()>().await;
        });

        // Convert receiver to stream and map messages
        let stream = UnboundedReceiverStream::new(receiver).filter_map(|msg| {
            match msg {
                Message::Bbo(bbo) => {
                    tracing::info!("Received BBO: {:?}", bbo);
                    Some(HyperliquidBbo {
                        coin: bbo.data.coin,
                        levels: bbo.data.bbo,
                        time: bbo.data.time,
                    })
                }
                _ => {
                    tracing::warn!("Received unexpected message type");
                    None
                }
            }
        });

        Ok(Box::pin(stream))
    }
}