use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use alloy::{
    primitives::{aliases::{U160, U24}, Address, U256},
    providers::Provider,
    signers::local::PrivateKeySigner,
    sol,
};

use crate::types::Executor;

sol! {
    #[sol(rpc)]
    interface ISwapRouter02 {
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }
        
        function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
        function multicall(uint256 deadline, bytes[] calldata data) external payable returns (bytes[] memory results);
    }
}

#[derive(Debug, Clone)]
pub struct UniV3SwapAction {
    pub token_in: Address,
    pub token_out: Address,
    pub fee: u32,
    pub amount_in: U256,
    pub amount_out_min: U256,
}

pub struct UniV3Executor<P> {
    provider: Arc<P>,
    signer: PrivateKeySigner,
    router_address: Address,
}

impl<P: Provider + 'static> UniV3Executor<P> {
    pub fn new(provider: Arc<P>, private_key: &str, router_address: Address) -> Result<Self> {
        let signer = private_key.parse::<PrivateKeySigner>()?;
        Ok(Self {
            provider,
            signer,
            router_address,
        })
    }
}

#[async_trait]
impl<P: Provider + 'static> Executor<UniV3SwapAction> for UniV3Executor<P> {
    async fn execute(&self, action: UniV3SwapAction) -> Result<()> {
        let owner = self.signer.address();
        
        let deadline = U256::from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() + 300
        );

        let params = ISwapRouter02::ExactInputSingleParams {
            tokenIn: action.token_in,
            tokenOut: action.token_out,
            fee: U24::from(action.fee),
            recipient: owner,
            amountIn: action.amount_in,
            amountOutMinimum: action.amount_out_min,
            sqrtPriceLimitX96: U160::ZERO,
        };

        let router = ISwapRouter02::new(self.router_address, &*self.provider);
        let encoded_call = router.exactInputSingle(params).calldata().to_owned();
        let multicall_data = vec![encoded_call.into()];
        
        let pending_tx = router
            .multicall(deadline, multicall_data)
            .from(owner)
            .gas(500_000)
            .send()
            .await?;
        
        let _tx_hash = *pending_tx.tx_hash();
        
        Ok(())
    }
}
