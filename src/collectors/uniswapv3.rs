use std::sync::Arc;

use alloy::{
    primitives::Address,
    providers::Provider,
};

use amms::{
    amms::{amm::AMM, uniswap_v3::UniswapV3Pool},
    state_space::StateSpaceBuilder,
};
use anyhow::Result;
use async_trait::async_trait;
use tokio_stream::StreamExt;

use crate::types::{Collector, CollectorStream};

/// Collector for Uniswap V3 style pools (like Hyperswap on Hyperliquid)
pub struct UniV3Collector<P> {
    provider: Arc<P>,
    pool_address: Address,
}

impl<P> UniV3Collector<P> {
    pub fn new(provider: Arc<P>, pool_address: Address) -> Self {
        Self {
            provider,
            pool_address,
        }
    }
}

#[async_trait]
impl<P> Collector<Vec<Address>> for UniV3Collector<P>
where
    P: Provider + 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Vec<Address>>> {
        tracing::info!("Initializing UniV3Collector for pool: {:?}", self.pool_address);
        
        // Create a single UniswapV3Pool AMM to track
        let pool: AMM = UniswapV3Pool::new(self.pool_address).into();
        let amms = vec![pool];

        tracing::info!("Building state space and syncing pool...");
        
        // Build state space and sync the pool
        let state_space_manager = StateSpaceBuilder::new(self.provider.clone())
            .with_amms(amms)
            .sync()
            .await
            .map_err(|e| {
                tracing::error!("Failed to sync state space: {}", e);
                e
            })?;

        tracing::info!("Successfully synced pool state");

        // state_space_manager.subscribe()

        // Subscribe to pool updates
        let stream = state_space_manager.subscribe().await?;

        // Map the stream to extract updated AMM addresses
        let stream = stream.map(|result| {
            result.unwrap_or_else(|e| {
                tracing::error!("Error receiving pool update: {}", e);
                vec![]
            })
        });

        Ok(Box::pin(stream))
    }
}
