//! Conditional Token Framework -- Gnosis-style split / merge / redeem.
//!
//! Three core operations:
//!
//! * **Split**: lock `amount` of collateral, mint `amount` of every outcome.
//! * **Merge**: burn `amount` of every outcome, unlock `amount` of collateral.
//! * **Redeem**: burn `amount` of the winning outcome after resolution, unlock
//!   `amount` of collateral. Losing outcomes redeem to zero.
//!
//! This file is pure logic -- the Anchor program calls into these helpers from
//! its instruction handlers so the rules cannot drift between simulation and
//! mainnet.

use crate::errors::{OutcomeError, Result};
use crate::fixed::{fee_multiplier, BPS_FACTOR};
use crate::market::{MarketState, OutcomeMarket};
use crate::outcome_token::OutcomeShares;

/// Result of a CTF operation -- what to debit / credit on chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitResult {
    /// Collateral the user owes the vault.
    pub collateral_in: u64,
    /// Outcome shares to mint to the user, per outcome index.
    pub shares_out: u64,
    /// Fee deducted from the collateral (kept by the protocol for buyback).
    pub fee_taken: u64,
}

pub fn split_position(
    market: &mut OutcomeMarket,
    holder_shares: &mut OutcomeShares,
    amount: u64,
) -> Result<SplitResult> {
    if amount == 0 {
        return Err(OutcomeError::ZeroAmount);
    }
    market.ensure_open()?;

    let (net, fee) = apply_fee(amount, market.fee_bps)?;
    for outcome in 0..market.outcome_count {
        holder_shares.add(outcome, net)?;
    }
    market.record_volume(amount);

    Ok(SplitResult {
        collateral_in: amount,
        shares_out: net,
        fee_taken: fee,
    })
}

pub fn merge_positions(
    market: &mut OutcomeMarket,
    holder_shares: &mut OutcomeShares,
    amount: u64,
) -> Result<u64> {
    if amount == 0 {
        return Err(OutcomeError::ZeroAmount);
    }
    if matches!(market.state, MarketState::Resolved { .. }) {
        return Err(OutcomeError::MarketClosed);
    }

    for outcome in 0..market.outcome_count {
        holder_shares.sub(outcome, amount)?;
    }
    market.record_volume(amount);
    Ok(amount)
}

pub fn redeem_payout(
    market: &mut OutcomeMarket,
    holder_shares: &mut OutcomeShares,
) -> Result<u64> {
    let winner = market.ensure_resolved()?;
    let payout = holder_shares.balance(winner)?;
    if payout == 0 {
        return Ok(0);
    }
    holder_shares.sub(winner, payout)?;
    // Losing outcomes are simply discarded -- they are worthless after
    // resolution and the vault keeps the collateral that backed them.
    market.record_volume(payout);
    Ok(payout)
}

fn apply_fee(amount: u64, fee_bps: u16) -> Result<(u64, u64)> {
    let mult = fee_multiplier(fee_bps);
    let net = (amount as u128)
        .checked_mul(mult)
        .and_then(|p| p.checked_div(BPS_FACTOR))
        .ok_or(OutcomeError::FixedPointOverflow)?;
    let net_u64: u64 = net.try_into().map_err(|_| OutcomeError::FixedPointOverflow)?;
    let fee = amount.saturating_sub(net_u64);
    Ok((net_u64, fee))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::{ResolutionSource};
    use crate::outcome_token::MAX_OUTCOMES;

    fn open_market(outcome_count: u8, fee_bps: u16) -> OutcomeMarket {
        OutcomeMarket {
            market_id: 1,
            authority: [0; 32],
            question: String::from("test"),
            outcome_count,
            resolution_source: ResolutionSource::Manual { authority: [0; 32] },
            resolution_deadline: 1_000_000_000,
            state: MarketState::Open,
            collateral_mint: [0; 32],
            collateral_vault: [0; 32],
            outcome_mints: [[0; 32]; MAX_OUTCOMES as usize],
            lmsr_b: 1_000_000,
            total_volume: 0,
            fee_bps,
            created_at: 0,
        }
    }

    #[test]
    fn split_then_merge_returns_collateral() {
        let mut market = open_market(2, 0);
        let mut shares = OutcomeShares::new(2).unwrap();
        let split = split_position(&mut market, &mut shares, 1_000).unwrap();
        assert_eq!(split.collateral_in, 1_000);
        assert_eq!(split.shares_out, 1_000);
        assert_eq!(shares.merge_capacity(), 1_000);

        let merged = merge_positions(&mut market, &mut shares, 600).unwrap();
        assert_eq!(merged, 600);
        assert_eq!(shares.merge_capacity(), 400);
    }

    #[test]
    fn split_charges_fee() {
        let mut market = open_market(2, 100); // 1% fee
        let mut shares = OutcomeShares::new(2).unwrap();
        let split = split_position(&mut market, &mut shares, 10_000).unwrap();
        assert_eq!(split.fee_taken, 100);
        assert_eq!(split.shares_out, 9_900);
    }

    #[test]
    fn redeem_pays_winner_only() {
        let mut market = open_market(2, 0);
        let mut shares = OutcomeShares::new(2).unwrap();
        split_position(&mut market, &mut shares, 1_000).unwrap();
        market.state = MarketState::Resolved { winning_outcome: 0 };

        let payout = redeem_payout(&mut market, &mut shares).unwrap();
        assert_eq!(payout, 1_000);
        assert_eq!(shares.balance(0).unwrap(), 0);
        assert_eq!(shares.balance(1).unwrap(), 1_000); // losing side stays zero-value
    }

    #[test]
    fn merge_rejects_zero_amount() {
        let mut market = open_market(2, 0);
        let mut shares = OutcomeShares::new(2).unwrap();
        assert_eq!(
            merge_positions(&mut market, &mut shares, 0).unwrap_err(),
            OutcomeError::ZeroAmount
        );
    }

    #[test]
    fn cannot_split_after_close() {
        let mut market = open_market(2, 0);
        market.state = MarketState::Closed;
        let mut shares = OutcomeShares::new(2).unwrap();
        assert_eq!(
            split_position(&mut market, &mut shares, 100).unwrap_err(),
            OutcomeError::MarketClosed
        );
    }
}
