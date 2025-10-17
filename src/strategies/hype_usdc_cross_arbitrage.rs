use anyhow::Result;
use async_trait::async_trait;
use tracing::info;
use alloy::primitives::Address;

use crate::collectors::{
    hyperliquid::HyperliquidBbo,
    uniswapv3::UniV3PoolState,
};
use crate::config::StrategyConfig;
use crate::types::Strategy;

#[derive(Debug, Clone)]
pub enum Event {
    PoolUpdate(UniV3PoolState),
    HyperliquidBbo(HyperliquidBbo),
}

// Re-export for convenience
pub use crate::executors::arbitrage::ArbitrageAction as Action;

#[derive(Debug, Clone)]
pub struct HypeUsdcCrossArbitrage {
    hyperliquid_bbo: Option<HyperliquidBbo>,
    hyperswap_state: Option<UniV3PoolState>,
    // Fee and order configuration
    order_size_usd: f64,
    hl_maker_fee_bps: f64,  // e.g., 2.0 for 0.02% fee, -2.0 for 0.02% rebate
    dex_gas_fee_usd: f64,
    min_profit_bps: f64,
    slippage_bps: f64,
    // Token addresses for DEX swaps (used when execution is enabled)
    #[allow(dead_code)]
    usdc_address: Address,
    #[allow(dead_code)]
    hype_address: Address,
    #[allow(dead_code)]
    dex_fee: u32,
}

impl HypeUsdcCrossArbitrage {
    /// Create strategy from config (recommended)
    pub fn from_config(config: &StrategyConfig) -> Result<Self> {
        let usdc_address = config.token_a_address.parse()
            .map_err(|_| anyhow::anyhow!("Invalid token_a address"))?;
        let hype_address = config.token_b_address.parse()
            .map_err(|_| anyhow::anyhow!("Invalid token_b address"))?;

        Ok(Self {
            hyperliquid_bbo: None,
            hyperswap_state: None,
            order_size_usd: config.order_size_usd,
            hl_maker_fee_bps: config.hl_maker_fee_bps,
            dex_gas_fee_usd: config.dex_gas_fee_usd,
            min_profit_bps: config.min_profit_bps,
            slippage_bps: config.slippage_bps,
            usdc_address,
            hype_address,
            dex_fee: config.fee,
        })
    }

    /// Create strategy directly (for examples/tests)
    pub fn new(
        order_size_usd: f64,
        hl_maker_fee_bps: f64,
        dex_gas_fee_usd: f64,
        min_profit_bps: f64,
        usdc_address: Address,
        hype_address: Address,
        dex_fee: u32,
    ) -> Self {
        Self {
            hyperliquid_bbo: None,
            hyperswap_state: None,
            order_size_usd,
            hl_maker_fee_bps,
            dex_gas_fee_usd,
            min_profit_bps,
            slippage_bps: 50.0,  // Default for examples
            usdc_address,
            hype_address,
            dex_fee,
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

        let raw_bid = bbo.levels[0].as_ref()?.px.parse::<f64>().ok()?;
        let raw_ask = bbo.levels[1].as_ref()?.px.parse::<f64>().ok()?;

        // Apply maker fee to spread (like we do for DEX)
        // Convert bps to decimal: positive fee = cost, negative fee = rebate
        let hl_maker_fee = self.hl_maker_fee_bps / 10000.0;
        let bid = raw_bid * (1.0 - hl_maker_fee);
        let ask = raw_ask * (1.0 + hl_maker_fee);

        Some((bid, ask))
    }

    /// Calculate net profit in basis points after all fees
    fn calculate_net_profit_bps(&self, buy_price: f64, sell_price: f64) -> f64 {
        // Gross profit percentage (fees already in spread)
        let gross_profit_pct = (sell_price - buy_price) / buy_price;
        
        // DEX gas fee as percentage of trade
        let gas_fee_pct = self.dex_gas_fee_usd / self.order_size_usd;
        
        // Net profit percentage after gas fee
        let net_profit_pct = gross_profit_pct - gas_fee_pct;
        
        // Convert to basis points
        net_profit_pct * 10000.0
    }

    fn generate_action(&self, buy_dex: bool, dex_price: f64, hl_price: f64) -> Action {
        use alloy::primitives::U256;
        use crate::executors::{univ3::UniV3SwapAction, hyperliquid::HyperliquidOrderAction};
        
        let hype_amount_raw = self.order_size_usd / dex_price;
        let hype_amount = (hype_amount_raw * 10000.0).round() / 10000.0;
        let usdc_raw = (self.order_size_usd * 1_000_000.0) as u64;
        let hype_raw = U256::from((hype_amount * 1e18) as u128);
        
        // Get slippage from config
        if buy_dex {
            let hl_sell_price = hl_price * (1.0 - self.slippage_bps / 10000.0);
            
            Action {
                dex_swap: UniV3SwapAction {
                    token_in: self.usdc_address,
                    token_out: self.hype_address,
                    fee: self.dex_fee,
                    amount_in: U256::from(usdc_raw),
                    amount_out_min: U256::ZERO,
                },
                hl_order: HyperliquidOrderAction {
                    coin: "HYPE/USDC".to_string(),
                    is_buy: false,
                    size: hype_amount,
                    limit_px: hl_sell_price,
                },
                direction: "Buy DEX".to_string(),
            }
        } else {
            let hl_buy_price = hl_price * (1.0 + self.slippage_bps / 10000.0);
            
            Action {
                dex_swap: UniV3SwapAction {
                    token_in: self.hype_address,
                    token_out: self.usdc_address,
                    fee: self.dex_fee,
                    amount_in: hype_raw,
                    amount_out_min: U256::ZERO,
                },
                hl_order: HyperliquidOrderAction {
                    coin: "HYPE/USDC".to_string(),
                    is_buy: true,
                    size: hype_amount,
                    limit_px: hl_buy_price,
                },
                direction: "Buy HL".to_string(),
            }
        }
    }
    
    fn check_and_generate_actions(&mut self) -> Vec<Action> {
        let (hl_bbo, dex_state) = match (&self.hyperliquid_bbo, &self.hyperswap_state) {
            (Some(b), Some(d)) => (b, d),
            _ => return vec![],
        };

        let (dex_bid, dex_ask) = match self.calculate_dex_bid_ask(dex_state) {
            Some(p) => p,
            None => return vec![],
        };

        let (hl_bid, hl_ask) = match self.get_hyperliquid_prices(hl_bbo) {
            Some(p) => p,
            None => return vec![],
        };

        let net_profit_1_bps = self.calculate_net_profit_bps(dex_ask, hl_bid);
        let net_profit_2_bps = self.calculate_net_profit_bps(hl_ask, dex_bid);

        // Log spreads without slippage
        info!("DEX {:.3}/{:.3} | HL {:.3}/{:.3} | Net: {:+.2}%/{:+.2}%",
            dex_bid, dex_ask, hl_bid, hl_ask, net_profit_1_bps / 100.0, net_profit_2_bps / 100.0);

        if net_profit_1_bps > self.min_profit_bps {
            info!("🎯 EXEC: Buy DEX → Sell HL ({:.2} bps > {} bps threshold)", 
                net_profit_1_bps, self.min_profit_bps);
            return vec![self.generate_action(true, dex_ask, hl_bid)];
        }
        if net_profit_2_bps > self.min_profit_bps {
            info!("🎯 EXEC: Buy HL → Sell DEX ({:.2} bps > {} bps threshold)", 
                net_profit_2_bps, self.min_profit_bps);
            return vec![self.generate_action(false, dex_bid, hl_ask)];
        }

        vec![]
    }
}

#[async_trait]
impl Strategy<Event, Action> for HypeUsdcCrossArbitrage {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::PoolUpdate(state) => {
                self.hyperswap_state = Some(state);
            }
            Event::HyperliquidBbo(bbo) => {
                self.hyperliquid_bbo = Some(bbo);
            }
        }
        
        // Check for arbitrage opportunities and generate actions
        self.check_and_generate_actions()
    }
}

