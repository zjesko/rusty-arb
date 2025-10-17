use anyhow::Result;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub rpc_url_ws: String,
    pub max_concurrent: usize,
    pub cooldown_secs: u64,
    pub strategies: Vec<StrategyConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StrategyConfig {
    pub name: String,
    pub enabled: bool,
    // DEX
    pub pool_address: String,
    pub router_address: String,
    pub fee: u32,
    pub token_a_address: String,
    pub token_b_address: String,
    // CEX
    pub hyperliquid_coin: String,
    // Strategy params
    pub order_size_usd: f64,
    pub hl_maker_fee_bps: f64,
    pub dex_gas_fee_usd: f64,
    pub min_profit_bps: f64,
    pub slippage_bps: f64,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        
        // Simple env var substitution: replace ${VAR} with env value
        let content = Self::substitute_env_vars(&content)?;
        
        let config: Config = toml::from_str(&content)?;
        
        if config.max_concurrent == 0 {
            anyhow::bail!("max_concurrent must be > 0");
        }
        
        for strategy in &config.strategies {
            if strategy.enabled && strategy.order_size_usd <= 0.0 {
                anyhow::bail!("order_size_usd must be > 0 in strategy '{}'", strategy.name);
            }
        }
        
        Ok(config)
    }
    
    fn substitute_env_vars(content: &str) -> Result<String> {
        let mut result = content.to_string();
        while let Some(start) = result.find("${") {
            if let Some(end) = result[start..].find('}') {
                let var_name = &result[start + 2..start + end];
                let value = std::env::var(var_name)
                    .map_err(|_| anyhow::anyhow!("Environment variable {} not found (check your .env file)", var_name))?;
                result.replace_range(start..start + end + 1, &value);
            } else {
                break;
            }
        }
        Ok(result)
    }
}
