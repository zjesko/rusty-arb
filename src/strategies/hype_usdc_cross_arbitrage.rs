use anyhow::Result;
use async_trait::async_trait;
use tracing::info;

use crate::collectors::{
    hyperliquid::HyperliquidBbo,
    uniswapv3::UniV3PoolState,
};
use crate::types::Strategy;

#[derive(Debug, Clone)]
pub enum Event {
    PoolUpdate(UniV3PoolState),
    HyperliquidBbo(HyperliquidBbo),
}

#[derive(Debug, Clone)]
pub enum Action {}

#[derive(Debug, Clone)]
pub struct HypeUsdcCrossArbitrage {
    hyperliquid_bbo: Option<HyperliquidBbo>,
    hyperswap_state: Option<UniV3PoolState>,
    min_profit_pct: f64,
}

impl HypeUsdcCrossArbitrage {
    pub fn new() -> Self {
        Self {
            hyperliquid_bbo: None,
            hyperswap_state: None,
            min_profit_pct: 0.0001,
        }
    }

    fn calculate_dex_bid_ask(&self, state: &UniV3PoolState) -> Option<(f64, f64)> {
        let sqrt_price_bytes = state.sqrt_price.to_be_bytes::<32>();
        let sqrt_price = u128::from_be_bytes([
            sqrt_price_bytes[16], sqrt_price_bytes[17], sqrt_price_bytes[18], sqrt_price_bytes[19],
            sqrt_price_bytes[20], sqrt_price_bytes[21], sqrt_price_bytes[22], sqrt_price_bytes[23],
            sqrt_price_bytes[24], sqrt_price_bytes[25], sqrt_price_bytes[26], sqrt_price_bytes[27],
            sqrt_price_bytes[28], sqrt_price_bytes[29], sqrt_price_bytes[30], sqrt_price_bytes[31],
        ]) as f64;
        
        let q96 = 2_f64.powi(96);
        let base_price = (sqrt_price / q96).powi(2);
        let decimal_adjustment = 10_f64.powi(state.token_a_decimals as i32 - state.token_b_decimals as i32);
        let mid_price = base_price * decimal_adjustment;
        
        let fee_fraction = state.fee as f64 / 1_000_000.0;
        let bid = mid_price * (1.0 - fee_fraction / 2.0);
        let ask = mid_price * (1.0 + fee_fraction / 2.0);
        
        Some((bid, ask))
    }

    fn get_hyperliquid_prices(&self, bbo: &HyperliquidBbo) -> Option<(f64, f64)> {
        if bbo.levels.len() < 2 {
            return None;
        }

        let bid = bbo.levels[0].as_ref()?.px.parse::<f64>().ok()?;
        let ask = bbo.levels[1].as_ref()?.px.parse::<f64>().ok()?;

        Some((bid, ask))
    }

    fn check_arbitrage(&self) {
        let (hl_bbo, dex_state) = match (&self.hyperliquid_bbo, &self.hyperswap_state) {
            (Some(b), Some(d)) => (b, d),
            _ => return,
        };

        let (dex_bid, dex_ask) = match self.calculate_dex_bid_ask(dex_state) {
            Some(p) => p,
            None => return,
        };

        let (hl_bid, hl_ask) = match self.get_hyperliquid_prices(hl_bbo) {
            Some(p) => p,
            None => return,
        };

        info!("📊 DEX: {:.4}/{:.4} | HL: {:.4}/{:.4}", dex_bid, dex_ask, hl_bid, hl_ask);

        let profit_1 = ((hl_bid - dex_ask) / dex_ask) * 100.0;
        if profit_1 > self.min_profit_pct {
            info!("🚨 ARB: Buy DEX @ {:.4} → Sell CEX @ {:.4} | +{:.2}%", 
                dex_ask, hl_bid, profit_1);
        }

        let profit_2 = ((dex_bid - hl_ask) / hl_ask) * 100.0;
        if profit_2 > self.min_profit_pct {
            info!("🚨 ARB: Buy CEX @ {:.4} → Sell DEX @ {:.4} | +{:.2}%", 
                hl_ask, dex_bid, profit_2);
        }
    }
}

#[async_trait]
impl Strategy<Event, Action> for HypeUsdcCrossArbitrage {
    async fn sync_state(&mut self) -> Result<()> {
        info!("Strategy initialized | Min profit: {:.2}%", self.min_profit_pct);
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::PoolUpdate(state) => {
                self.hyperswap_state = Some(state);
                self.check_arbitrage();
            }
            Event::HyperliquidBbo(bbo) => {
                self.hyperliquid_bbo = Some(bbo);
                self.check_arbitrage();
            }
        }
        vec![]
    }
}

