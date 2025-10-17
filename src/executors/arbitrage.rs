use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use alloy::providers::Provider;
use tracing::{error, info};

use crate::execution::ExecutionManager;
use crate::executors::{
    univ3::{UniV3Executor, UniV3SwapAction},
    hyperliquid::{HyperliquidExecutor, HyperliquidOrderAction},
};
use crate::types::Executor;

/// Action for executing complete arbitrage (both legs)
#[derive(Debug, Clone)]
pub struct ArbitrageAction {
    pub dex_swap: UniV3SwapAction,
    pub hl_order: HyperliquidOrderAction,
    pub direction: String,
}

/// Composite executor that handles both DEX and HL legs sequentially
pub struct ArbitrageExecutor<P> {
    dex_executor: UniV3Executor<P>,
    hl_executor: HyperliquidExecutor,
    exec_manager: Arc<ExecutionManager>,
    cooldown_secs: u64,
}

impl<P> ArbitrageExecutor<P> {
    pub fn new(
        dex_executor: UniV3Executor<P>,
        hl_executor: HyperliquidExecutor,
        exec_manager: Arc<ExecutionManager>,
        cooldown_secs: u64,
    ) -> Self {
        Self {
            dex_executor,
            hl_executor,
            exec_manager,
            cooldown_secs,
        }
    }
}

#[async_trait]
impl<P: Provider + 'static> Executor<ArbitrageAction> for ArbitrageExecutor<P> {
    async fn execute(&self, action: ArbitrageAction) -> Result<()> {
        // Try to acquire execution permit
        let _permit = match self.exec_manager.try_start() {
            Some(p) => p,
            None => {
                info!("⏸️  Skipping {} - execution already in progress", action.direction);
                return Ok(());
            }
        };

        info!("🚀 {}", action.direction);

        // Execute DEX leg
        if let Err(e) = self.dex_executor.execute(action.dex_swap.clone()).await {
            error!("DEX failed: {}", e);
            return Err(e);
        }

        // Execute HL leg
        if let Err(e) = self.hl_executor.execute(action.hl_order.clone()).await {
            error!("HL failed: {} ⚠️ ONE-SIDED!", e);
            return Err(e);
        }

        // Log PnL
        Self::log_pnl(&action);

        // Cooldown
        tokio::time::sleep(tokio::time::Duration::from_secs(self.cooldown_secs)).await;

        // Permit auto-releases here via Drop
        Ok(())
    }
}

impl<P> ArbitrageExecutor<P> {
    fn log_pnl(action: &ArbitrageAction) {
        let trade_size = action.hl_order.size * action.hl_order.limit_px;
        let dex_fee = trade_size * 0.003;
        let hl_fee = trade_size * 0.0002;
        let gas = 0.50;
        let total_fees = dex_fee + hl_fee + gas;
        
        info!("💰 Size: ${:.1} | Fees: ${:.2} (DEX ${:.2} + HL ${:.2} + Gas ${:.2})",
            trade_size, total_fees, dex_fee, hl_fee, gas);
    }
}

