//! Logarithmic Market Scoring Rule (LMSR) -- Hanson (2003).
//!
//! Reference: Robin Hanson, "Combinatorial Information Market Design", 2003.
//!
//! Pricing follows the canonical cost function:
//!
//! ```text
//!     C(q) = b * ln( Σ exp(q_i / b) )
//!     p_i  = exp(q_i / b) / Σ exp(q_j / b)
//! ```
//!
//! Buying `Δ` shares of outcome `i` costs `C(q + Δ·e_i) − C(q)`. The `b`
//! parameter (liquidity) is fixed per market and bounds the worst-case loss
//! of the AMM at `b · ln(N)`.
//!
//! All math is f64 -- on chain we project back to u64 via `BPS_FACTOR`. The
//! crate is `no_std` capable for re-use inside the Anchor program (where we
//! drop `f64` for a fixed-point implementation).

use std::f64::consts::E;
use thiserror::Error;
use wagr_outcome_engine::fixed::BPS_FACTOR;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LmsrError {
    #[error("share vector empty")]
    EmptyShares,
    #[error("outcome index out of range")]
    OutOfRange,
    #[error("b parameter must be positive")]
    InvalidLiquidity,
    #[error("numerical overflow -- consider increasing b")]
    Overflow,
}

/// Numerically stable `logsumexp(xs)` = `m + ln( Σ exp(xs_i − m) )`.
/// Avoids the overflow that a naive `ln(Σ exp(x))` hits once any `x > ~700`.
pub fn logsumexp(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return f64::NEG_INFINITY;
    }
    let m = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if m.is_infinite() {
        return m;
    }
    let mut sum = 0.0_f64;
    for x in xs {
        sum += (x - m).exp();
    }
    m + sum.ln()
}

/// LMSR cost C(q) for share vector `q` and liquidity `b`.
pub fn lmsr_cost(qs: &[f64], b: f64) -> Result<f64, LmsrError> {
    if qs.is_empty() {
        return Err(LmsrError::EmptyShares);
    }
    if b <= 0.0 {
        return Err(LmsrError::InvalidLiquidity);
    }
    let scaled: Vec<f64> = qs.iter().map(|q| q / b).collect();
    Ok(b * logsumexp(&scaled))
}

/// Implied price for outcome `i`. Always in [0, 1] with `Σ p_i = 1`.
pub fn lmsr_price(qs: &[f64], b: f64, outcome: usize) -> Result<f64, LmsrError> {
    if outcome >= qs.len() {
        return Err(LmsrError::OutOfRange);
    }
    let scaled: Vec<f64> = qs.iter().map(|q| q / b).collect();
    let m = scaled.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut sum = 0.0_f64;
    let mut top = 0.0_f64;
    for (i, x) in scaled.iter().enumerate() {
        let e = (x - m).exp();
        sum += e;
        if i == outcome {
            top = e;
        }
    }
    if sum == 0.0 {
        return Err(LmsrError::Overflow);
    }
    Ok(top / sum)
}

/// Cost to buy `shares` of outcome `i` -- `C(q + Δ·e_i) − C(q)`.
pub fn lmsr_buy_quote(
    qs: &[f64],
    b: f64,
    outcome: usize,
    shares: f64,
) -> Result<f64, LmsrError> {
    if outcome >= qs.len() {
        return Err(LmsrError::OutOfRange);
    }
    if shares < 0.0 {
        return Err(LmsrError::InvalidLiquidity);
    }
    let mut next: Vec<f64> = qs.to_vec();
    next[outcome] += shares;
    Ok(lmsr_cost(&next, b)? - lmsr_cost(qs, b)?)
}

/// Collateral returned for selling `shares` of outcome `i`.
pub fn lmsr_sell_quote(
    qs: &[f64],
    b: f64,
    outcome: usize,
    shares: f64,
) -> Result<f64, LmsrError> {
    if outcome >= qs.len() {
        return Err(LmsrError::OutOfRange);
    }
    if shares < 0.0 || qs[outcome] < shares {
        return Err(LmsrError::InvalidLiquidity);
    }
    let mut next: Vec<f64> = qs.to_vec();
    next[outcome] -= shares;
    Ok(lmsr_cost(qs, b)? - lmsr_cost(&next, b)?)
}

/// Maximum worst-case AMM loss for an N-outcome market: `b · ln(N)`.
pub fn lmsr_max_loss(b: f64, outcomes: usize) -> f64 {
    b * (outcomes as f64).ln()
}

/// CPMM fallback used when liquidity > `cpmm_threshold` so trades stop paying
/// the `exp` cost. Uses a constant-product invariant on the two-outcome case.
pub fn cpmm_price(reserve_yes: f64, reserve_no: f64) -> f64 {
    if reserve_yes + reserve_no == 0.0 {
        return 0.5;
    }
    reserve_no / (reserve_yes + reserve_no)
}

/// Convert an f64 fixed-point amount into the chain's u128 representation.
pub fn to_chain_units(value: f64) -> Option<u128> {
    let scaled = value * BPS_FACTOR as f64;
    if !scaled.is_finite() || scaled < 0.0 {
        return None;
    }
    Some(scaled.round() as u128)
}

/// e^1 -- handy for derivative tests.
pub const E_F64: f64 = E;

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn binary_prices_sum_to_one() {
        let qs = [0.0_f64, 0.0];
        let b = 1000.0;
        let p0 = lmsr_price(&qs, b, 0).unwrap();
        let p1 = lmsr_price(&qs, b, 1).unwrap();
        assert_abs_diff_eq!(p0 + p1, 1.0, epsilon = 1e-9);
        assert_abs_diff_eq!(p0, 0.5, epsilon = 1e-9);
    }

    #[test]
    fn buy_then_sell_is_lossless_at_zero_fee() {
        let mut qs = [0.0_f64, 0.0];
        let b = 500.0;
        let cost = lmsr_buy_quote(&qs, b, 0, 100.0).unwrap();
        qs[0] += 100.0;
        let refund = lmsr_sell_quote(&qs, b, 0, 100.0).unwrap();
        assert_abs_diff_eq!(cost, refund, epsilon = 1e-6);
    }

    #[test]
    fn max_loss_matches_paper() {
        let loss = lmsr_max_loss(500.0, 2);
        assert_abs_diff_eq!(loss, 500.0 * 2.0_f64.ln(), epsilon = 1e-12);
    }

    #[test]
    fn logsumexp_handles_overflow() {
        let xs = [1000.0_f64, 1000.0, 1000.0];
        let lse = logsumexp(&xs);
        // log( 3 e^1000 ) = 1000 + ln(3)
        assert_abs_diff_eq!(lse, 1000.0 + 3.0_f64.ln(), epsilon = 1e-9);
    }

    #[test]
    fn n_outcome_prices_sum_to_one() {
        let qs = [10.0, -20.0, 5.0, 0.0_f64];
        let b = 100.0;
        let sum: f64 = (0..4)
            .map(|i| lmsr_price(&qs, b, i).unwrap())
            .sum();
        assert_abs_diff_eq!(sum, 1.0, epsilon = 1e-9);
    }
}
