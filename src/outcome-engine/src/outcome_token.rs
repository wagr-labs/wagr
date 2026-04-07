//! Outcome Token accounting.
//!
//! Solana port of the Polymarket binary-outcome ERC-1155 pattern. Each market
//! mints one SPL Token-2022 mint per outcome. Off-chain we represent the
//! per-trader balance with [`OutcomeShares`], which mirrors the layout we use
//! inside the Anchor account.

use crate::errors::{OutcomeError, Result};

/// Hard cap on the number of outcomes per market.
/// Binary == 2. Election shortlist == up to 16.
pub const MAX_OUTCOMES: u8 = 16;

/// Per-outcome share balance for a single holder.
///
/// `shares[i]` is the number of "outcome i" tokens held. The vector is always
/// sized to `outcome_count` so a binary market never carries unused trailing
/// zeros on chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutcomeShares {
    pub outcome_count: u8,
    pub shares: [u64; MAX_OUTCOMES as usize],
}

impl OutcomeShares {
    pub fn new(outcome_count: u8) -> Result<Self> {
        if !(2..=MAX_OUTCOMES).contains(&outcome_count) {
            return Err(OutcomeError::InvalidOutcomeCount { max: MAX_OUTCOMES });
        }
        Ok(Self {
            outcome_count,
            shares: [0u64; MAX_OUTCOMES as usize],
        })
    }

    pub fn balance(&self, outcome: u8) -> Result<u64> {
        self.check_index(outcome)?;
        Ok(self.shares[outcome as usize])
    }

    pub fn add(&mut self, outcome: u8, amount: u64) -> Result<()> {
        self.check_index(outcome)?;
        if amount == 0 {
            return Err(OutcomeError::ZeroAmount);
        }
        let slot = &mut self.shares[outcome as usize];
        *slot = slot
            .checked_add(amount)
            .ok_or(OutcomeError::BalanceOverflow { outcome })?;
        Ok(())
    }

    pub fn sub(&mut self, outcome: u8, amount: u64) -> Result<()> {
        self.check_index(outcome)?;
        if amount == 0 {
            return Err(OutcomeError::ZeroAmount);
        }
        let slot = &mut self.shares[outcome as usize];
        *slot = slot
            .checked_sub(amount)
            .ok_or(OutcomeError::BalanceUnderflow { outcome })?;
        Ok(())
    }

    /// Total exposure across every outcome -- useful for net merge accounting.
    pub fn total(&self) -> u64 {
        self.shares
            .iter()
            .take(self.outcome_count as usize)
            .copied()
            .fold(0u64, |acc, v| acc.saturating_add(v))
    }

    /// Minimum share across every outcome -- this is exactly the amount of
    /// collateral that can be reclaimed via Gnosis-style merge.
    pub fn merge_capacity(&self) -> u64 {
        self.shares
            .iter()
            .take(self.outcome_count as usize)
            .copied()
            .min()
            .unwrap_or(0)
    }

    fn check_index(&self, outcome: u8) -> Result<()> {
        if outcome >= self.outcome_count {
            return Err(OutcomeError::OutcomeOutOfRange {
                index: outcome,
                count: self.outcome_count,
            });
        }
        Ok(())
    }
}

/// Aggregate balance information returned to clients. Lighter than passing the
/// raw [`OutcomeShares`] across an FFI boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutcomeBalance {
    pub outcome: u8,
    pub shares: u64,
}

/// Mint identity for a single outcome -- mirrors the on-chain account layout
/// without pulling in Solana types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutcomeMint {
    pub outcome: u8,
    pub mint: [u8; 32],
    pub supply: u64,
    pub decimals: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_split_round_trip() {
        let mut bal = OutcomeShares::new(2).unwrap();
        bal.add(0, 100).unwrap();
        bal.add(1, 100).unwrap();
        assert_eq!(bal.total(), 200);
        assert_eq!(bal.merge_capacity(), 100);
        bal.sub(0, 60).unwrap();
        assert_eq!(bal.merge_capacity(), 40);
    }

    #[test]
    fn rejects_invalid_outcome_count() {
        assert!(OutcomeShares::new(1).is_err());
        assert!(OutcomeShares::new(MAX_OUTCOMES + 1).is_err());
        assert!(OutcomeShares::new(2).is_ok());
        assert!(OutcomeShares::new(MAX_OUTCOMES).is_ok());
    }

    #[test]
    fn rejects_out_of_range_outcome() {
        let mut bal = OutcomeShares::new(3).unwrap();
        assert_eq!(
            bal.add(3, 10).unwrap_err(),
            OutcomeError::OutcomeOutOfRange { index: 3, count: 3 }
        );
    }

    #[test]
    fn rejects_zero_amount() {
        let mut bal = OutcomeShares::new(2).unwrap();
        assert_eq!(bal.add(0, 0).unwrap_err(), OutcomeError::ZeroAmount);
    }

    #[test]
    fn rejects_underflow() {
        let mut bal = OutcomeShares::new(2).unwrap();
        bal.add(0, 10).unwrap();
        assert!(matches!(
            bal.sub(0, 11).unwrap_err(),
            OutcomeError::BalanceUnderflow { .. }
        ));
    }
}
