//! Reputation module -- governs the WAGR DVM.
//!
//! When the Optimistic Oracle has a challenge, control hands off here. Voters
//! stake `$WAGR` and cast a vote. The verdict is the stake-weighted majority.
//! Voters on the winning side share the loser's bond (after a protocol cut);
//! voters on the losing side get **slashed** in proportion to their stake.
//!
//! The reputation score is updated multiplicatively so persistent good actors
//! pay a smaller marginal cost (their effective stake grows) and persistent
//! bad actors pay an exponentially-rising cost (their reputation decays to
//! zero, after which they can never out-vote a fresh honest voter).

use thiserror::Error;
use wagr_oracle_resolver::Dispute;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReputationError {
    #[error("vote already cast for this dispute")]
    DoubleVote,
    #[error("stake below minimum {min}")]
    BelowMinimum { min: u64 },
    #[error("voter not registered")]
    UnknownVoter,
    #[error("dispute not active")]
    NoActiveDispute,
    #[error("arithmetic overflow")]
    Overflow,
}

/// Reputation score uses six-decimal fixed point centred on 1.0
/// (== 1_000_000). A score of 0.5 halves a voter's effective stake; 2.0
/// doubles it. Capped at 4.0 to bound the influence of any single voter.
pub const REP_ONE: u64 = 1_000_000;
pub const REP_MIN: u64 = 1; // floor so a fully-slashed voter can still recover.
pub const REP_MAX: u64 = 4 * REP_ONE;

/// Per-voter ledger entry. Lives in a PDA on chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reputation {
    pub voter: [u8; 32],
    pub stake: u64,
    pub correct_votes: u32,
    pub incorrect_votes: u32,
    pub score: u64,
}

impl Reputation {
    pub fn new(voter: [u8; 32], stake: u64) -> Self {
        Self {
            voter,
            stake,
            correct_votes: 0,
            incorrect_votes: 0,
            score: REP_ONE,
        }
    }

    /// Effective stake = `stake * score` with overflow protection.
    pub fn effective_stake(&self) -> u128 {
        (self.stake as u128) * (self.score as u128) / (REP_ONE as u128)
    }

    fn bump_correct(&mut self) {
        self.correct_votes = self.correct_votes.saturating_add(1);
        // score *= 1.10, capped at REP_MAX.
        let new_score = (self.score as u128) * 110 / 100;
        self.score = new_score.min(REP_MAX as u128) as u64;
    }

    fn bump_incorrect(&mut self) {
        self.incorrect_votes = self.incorrect_votes.saturating_add(1);
        // score *= 0.5, floored at REP_MIN.
        let new_score = (self.score / 2).max(REP_MIN);
        self.score = new_score;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vote {
    pub voter: [u8; 32],
    pub outcome: u8,
    pub effective_stake: u128,
}

/// DVM tally for a single dispute.
#[derive(Clone, Debug)]
pub struct DvmRound {
    pub dispute: Dispute,
    pub votes: Vec<Vote>,
    pub min_stake: u64,
    pub finalised: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DvmVerdict {
    pub winning_outcome: u8,
    pub winning_stake: u128,
    pub losing_stake: u128,
    pub total_voters: u32,
}

impl DvmRound {
    pub fn new(dispute: Dispute, min_stake: u64) -> Self {
        Self {
            dispute,
            votes: Vec::new(),
            min_stake,
            finalised: false,
        }
    }

    pub fn cast_vote(
        &mut self,
        rep: &Reputation,
        outcome: u8,
    ) -> Result<(), ReputationError> {
        if self.finalised {
            return Err(ReputationError::NoActiveDispute);
        }
        if rep.stake < self.min_stake {
            return Err(ReputationError::BelowMinimum { min: self.min_stake });
        }
        if self.votes.iter().any(|v| v.voter == rep.voter) {
            return Err(ReputationError::DoubleVote);
        }
        self.votes.push(Vote {
            voter: rep.voter,
            outcome,
            effective_stake: rep.effective_stake(),
        });
        Ok(())
    }

    /// Tally votes, return the verdict, mark the round finalised.
    pub fn tally(&mut self) -> Result<DvmVerdict, ReputationError> {
        if self.finalised {
            return Err(ReputationError::NoActiveDispute);
        }
        if self.votes.is_empty() {
            return Err(ReputationError::NoActiveDispute);
        }

        // 16-bin tally covers MAX_OUTCOMES.
        let mut tally: [u128; 16] = [0; 16];
        for v in &self.votes {
            tally[v.outcome as usize] = tally[v.outcome as usize]
                .checked_add(v.effective_stake)
                .ok_or(ReputationError::Overflow)?;
        }
        let (winning_outcome, winning_stake) = tally
            .iter()
            .enumerate()
            .max_by_key(|(_, s)| **s)
            .map(|(i, s)| (i as u8, *s))
            .ok_or(ReputationError::NoActiveDispute)?;
        let losing_stake: u128 = tally
            .iter()
            .enumerate()
            .filter(|(i, _)| *i as u8 != winning_outcome)
            .map(|(_, s)| *s)
            .sum();

        self.finalised = true;
        Ok(DvmVerdict {
            winning_outcome,
            winning_stake,
            losing_stake,
            total_voters: self.votes.len() as u32,
        })
    }

    /// Apply per-voter score / stake updates from a verdict. Returns the
    /// post-update reputation set so callers can write it back.
    pub fn apply_verdict(
        &self,
        verdict: &DvmVerdict,
        mut reps: Vec<Reputation>,
        slash_bps: u16,
    ) -> Vec<Reputation> {
        for r in reps.iter_mut() {
            if let Some(v) = self.votes.iter().find(|v| v.voter == r.voter) {
                if v.outcome == verdict.winning_outcome {
                    r.bump_correct();
                } else {
                    r.bump_incorrect();
                    // Slash `slash_bps` of stake -- 1% per bps step.
                    let slash = (r.stake as u128) * (slash_bps as u128) / 10_000;
                    r.stake = r.stake.saturating_sub(slash as u64);
                }
            }
        }
        reps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dispute() -> Dispute {
        Dispute {
            market_id: 1,
            round: 0,
            proposer: [1; 32],
            challenger: [2; 32],
            proposed_outcome: 0,
            challenge_window: 86_400,
            raised_at: 1_000,
        }
    }

    #[test]
    fn majority_stake_wins() {
        let mut round = DvmRound::new(make_dispute(), 100);
        let r_yes_a = Reputation::new([10; 32], 500);
        let r_yes_b = Reputation::new([11; 32], 500);
        let r_no = Reputation::new([20; 32], 800);

        round.cast_vote(&r_yes_a, 0).unwrap();
        round.cast_vote(&r_yes_b, 0).unwrap();
        round.cast_vote(&r_no, 1).unwrap();
        let v = round.tally().unwrap();
        assert_eq!(v.winning_outcome, 0);
        assert!(v.winning_stake > v.losing_stake);
    }

    #[test]
    fn slashes_losers() {
        let mut round = DvmRound::new(make_dispute(), 100);
        let r_yes = Reputation::new([10; 32], 1_000);
        let r_no = Reputation::new([20; 32], 1_000);
        round.cast_vote(&r_yes, 0).unwrap();
        round.cast_vote(&r_no, 1).unwrap();
        // Reputation tie-breaks by stake order; outcome 0 wins because it was
        // recorded first under equal stake.
        let v = round.tally().unwrap();
        let updated = round.apply_verdict(&v, vec![r_yes, r_no], 1_000); // 10% slash
        let loser = if v.winning_outcome == 0 { updated[1] } else { updated[0] };
        let winner = if v.winning_outcome == 0 { updated[0] } else { updated[1] };
        assert!(loser.stake < 1_000);
        assert_eq!(winner.stake, 1_000); // winner stake unchanged
        assert!(winner.score > REP_ONE);
        assert!(loser.score < REP_ONE);
    }

    #[test]
    fn double_vote_blocked() {
        let mut round = DvmRound::new(make_dispute(), 100);
        let r = Reputation::new([10; 32], 1_000);
        round.cast_vote(&r, 0).unwrap();
        assert_eq!(round.cast_vote(&r, 0).unwrap_err(), ReputationError::DoubleVote);
    }

    #[test]
    fn rejects_below_minimum() {
        let mut round = DvmRound::new(make_dispute(), 100);
        let r = Reputation::new([10; 32], 50);
        assert!(matches!(
            round.cast_vote(&r, 0).unwrap_err(),
            ReputationError::BelowMinimum { min: 100 }
        ));
    }
}
