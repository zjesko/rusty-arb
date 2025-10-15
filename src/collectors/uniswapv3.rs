use std::sync::Arc;

use alloy::{
    primitives::{Address, U256},
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

#[derive(Debug, Clone)]
pub struct UniV3PoolState {
    pub sqrt_price: U256,
    pub fee: u32,
    pub token_a_decimals: u8,
    pub token_b_decimals: u8,
}

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

    fn extract_pool_state(pool: &UniswapV3Pool, _address: Address) -> UniV3PoolState {
        UniV3PoolState {
            sqrt_price: pool.sqrt_price,
            fee: pool.fee,
            token_a_decimals: pool.token_a.decimals,
            token_b_decimals: pool.token_b.decimals,
        }
    }
}

#[async_trait]
impl<P> Collector<UniV3PoolState> for UniV3Collector<P>
where
    P: Provider + 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, UniV3PoolState>> {
        let pool: AMM = UniswapV3Pool::new(self.pool_address).into();
        
        let state_space_manager = StateSpaceBuilder::new(self.provider.clone())
            .with_amms(vec![pool])
            .sync()
            .await?;

        let state = state_space_manager.state.clone();
        
        let initial_state = {
            let state_guard = state.read().await;
            state_guard.get(&self.pool_address)
                .and_then(|amm| {
                    if let AMM::UniswapV3Pool(pool) = amm {
                        Some(Self::extract_pool_state(pool, self.pool_address))
                    } else {
                        None
                    }
                })
        };

        let stream = state_space_manager.subscribe().await?;

        let updates_stream = stream.then(move |result| {
            let state = state.clone();
            async move {
                match result {
                    Ok(addresses) => {
                        if addresses.is_empty() {
                            return None;
                        }
                        
                        let address = addresses[0];
                        let state_guard = state.read().await;
                        state_guard.get(&address)
                            .and_then(|amm| {
                                if let AMM::UniswapV3Pool(pool) = amm {
                                    Some(Self::extract_pool_state(pool, address))
                                } else {
                                    None
                                }
                            })
                    }
                    Err(_) => None,
                }
            }
        }).filter_map(|x| x);
        
        let combined_stream = tokio_stream::iter(initial_state).chain(updates_stream);

        Ok(Box::pin(combined_stream))
    }
}
