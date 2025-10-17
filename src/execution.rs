use std::sync::Arc;
use tokio::sync::{Semaphore, OwnedSemaphorePermit};

/// Manages execution concurrency across all arbitrage strategies
#[derive(Clone)]
pub struct ExecutionManager {
    semaphore: Arc<Semaphore>,
}

impl ExecutionManager {
    /// Create new execution manager with max concurrent executions
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Try to start execution (non-blocking)
    pub fn try_start(&self) -> Option<ExecutionPermit> {
        self.semaphore.clone().try_acquire_owned().ok().map(|permit| {
            ExecutionPermit { _permit: permit }
        })
    }
}

/// RAII permit - auto-releases on drop
pub struct ExecutionPermit {
    _permit: OwnedSemaphorePermit,
}

