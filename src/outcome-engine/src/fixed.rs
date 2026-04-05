//! Fixed-point helpers shared by outcome / LMSR / oracle code.
//!
//! We standardise on **u128 with BPS_FACTOR = 1_000_000** (six decimal places)
//! for prices and shares. Token amounts use the underlying SPL decimal
//! convention (usually 6 for USDC). All arithmetic is checked for overflow.

use crate::errors::OutcomeError;

/// Six-decimal fixed-point factor.
pub const BPS_FACTOR: u128 = 1_000_000;
/// Maximum representable price in fixed-point form (== 1.0).
pub const PRICE_ONE: u128 = BPS_FACTOR;

/// Multiply two fixed-point numbers, dividing back by BPS_FACTOR.
pub fn fixed_mul(a: u128, b: u128) -> Result<u128, OutcomeError> {
    a.checked_mul(b)
        .and_then(|p| p.checked_div(BPS_FACTOR))
        .ok_or(OutcomeError::FixedPointOverflow)
}

/// Divide a by b, scaling so the result stays in fixed-point.
pub fn fixed_div(a: u128, b: u128) -> Result<u128, OutcomeError> {
    a.checked_mul(BPS_FACTOR)
        .and_then(|p| p.checked_div(b.max(1)))
        .ok_or(OutcomeError::FixedPointOverflow)
}

/// Convert a basis-point fee (out of 10_000) into a fixed-point multiplier.
pub fn fee_multiplier(fee_bps: u16) -> u128 {
    let fee = (fee_bps as u128) * 100; // bps -> 1e6 units (1bps == 100 micro)
    BPS_FACTOR.saturating_sub(fee)
}
