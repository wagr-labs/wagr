//! Market lifecycle state machine.
//!
//! Mirrors the Anchor account layout so the same enum drives both off-chain
//! simulation and on-chain enforcement.

use crate::errors::{OutcomeError, Result};
use crate::outcome_token::MAX_OUTCOMES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketState {
    /// Liquidity bootstrapped, accepting trades.
    Open,
    /// Trading window closed, awaiting oracle.
    Closed,
    /// Oracle decided no valid outcome exists -- collateral refunded.
    ResolvedInvalid,
    /// Oracle decided. `winning_outcome` is canonical.
    Resolved { winning_outcome: u8 },
    /// A challenge has been raised; resolution paused.
    Disputed,
}

/// Resolution source -- matches the Anchor enum byte for byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolutionSource {
    /// UMA-style optimistic resolution -- bonded proposal + challenge window.
    UmaOptimistic { bond: u64, challenge_window: i64 },
    /// Pull a price feed and threshold it.
    PythAggregator { feed: [u8; 32], threshold: i64 },
    /// Authority signs off (test markets, off-chain games).
    Manual { authority: [u8; 32] },
    /// M-of-N multi-source consensus.
    MultiSourceConsensus { sources: u8, threshold: u8 },
}

/// Off-chain projection of the on-chain `OutcomeMarket` account.
#[derive(Clone, Debug)]
pub struct OutcomeMarket {
    pub market_id: u64,
    pub authority: [u8; 32],
    pub question: String,
    pub outcome_count: u8,
    pub resolution_source: ResolutionSource,
    pub resolution_deadline: i64,
    pub state: MarketState,
    pub collateral_mint: [u8; 32],
    pub collateral_vault: [u8; 32],
    pub outcome_mints: [[u8; 32]; MAX_OUTCOMES as usize],
    pub lmsr_b: u128,
    pub total_volume: u64,
    pub fee_bps: u16,
    pub created_at: i64,
}

impl OutcomeMarket {
    pub fn ensure_open(&self) -> Result<()> {
        match self.state {
            MarketState::Open => Ok(()),
            MarketState::Closed | MarketState::Disputed => Err(OutcomeError::MarketClosed),
            MarketState::Resolved { .. } => Err(OutcomeError::MarketClosed),
            MarketState::ResolvedInvalid => Err(OutcomeError::MarketResolvedInvalid),
        }
    }

    pub fn ensure_resolved(&self) -> Result<u8> {
        match self.state {
            MarketState::Resolved { winning_outcome } => Ok(winning_outcome),
            MarketState::ResolvedInvalid => Err(OutcomeError::MarketResolvedInvalid),
            _ => Err(OutcomeError::MarketNotResolved),
        }
    }

    /// Move from Open -> Closed when the resolution window opens.
    pub fn close(&mut self, now: i64) -> Result<()> {
        if now < self.resolution_deadline {
            return Err(OutcomeError::DeadlinePassed);
        }
        if !matches!(self.state, MarketState::Open) {
            return Err(OutcomeError::MarketClosed);
        }
        self.state = MarketState::Closed;
        Ok(())
    }

    pub fn record_volume(&mut self, amount: u64) {
        self.total_volume = self.total_volume.saturating_add(amount);
    }
}
