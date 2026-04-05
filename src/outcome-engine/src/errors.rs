use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OutcomeError {
    #[error("market is closed -- accepts no new positions")]
    MarketClosed,
    #[error("market has not been resolved yet")]
    MarketNotResolved,
    #[error("market resolved invalid -- only merge / refund allowed")]
    MarketResolvedInvalid,
    #[error("outcome index {index} out of range (count={count})")]
    OutcomeOutOfRange { index: u8, count: u8 },
    #[error("outcome count must be between 2 and {max}")]
    InvalidOutcomeCount { max: u8 },
    #[error("balance underflow on outcome {outcome}")]
    BalanceUnderflow { outcome: u8 },
    #[error("balance overflow on outcome {outcome}")]
    BalanceOverflow { outcome: u8 },
    #[error("amount must be > 0")]
    ZeroAmount,
    #[error("resolution deadline already passed")]
    DeadlinePassed,
    #[error("collateral mismatch -- expected {expected}, got {got}")]
    CollateralMismatch { expected: u64, got: u64 },
    #[error("not authorized")]
    Unauthorized,
    #[error("fixed-point overflow")]
    FixedPointOverflow,
}

pub type Result<T> = core::result::Result<T, OutcomeError>;
