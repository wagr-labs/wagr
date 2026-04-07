pub mod errors;
pub mod fixed;
pub mod outcome_token;

pub use errors::OutcomeError;
pub use outcome_token::{OutcomeBalance, OutcomeMint, OutcomeShares, MAX_OUTCOMES};
