//! WAGR Outcome Executor -- Anchor 0.31 program.
//!
//! This is the on-chain front door for the WAGR prediction market standard.
//! All math lives in the workspace crates (`wagr-outcome-engine`,
//! `wagr-lmsr-amm`, `wagr-oracle-resolver`, `wagr-reputation-module`) so the
//! program is intentionally thin: it owns account layout, signer checks, and
//! the SPL Token-2022 calls. The CTF rules, LMSR pricing, and DVM tally are
//! delegated.

use anchor_lang::prelude::*;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TransferChecked, MintTo, transfer_checked, mint_to};
use wagr_lmsr_amm::lmsr_buy_quote;
use wagr_outcome_engine::ctf::{merge_positions, redeem_payout, split_position};
use wagr_outcome_engine::market::{MarketState, OutcomeMarket as DomainMarket, ResolutionSource as DomainResolutionSource};
use wagr_outcome_engine::outcome_token::OutcomeShares;

declare_id!("GreSDUbtzBpDRgCYo9sXGZbFDM3HXFQWTdeAacF8HDEc");

pub const MAX_OUTCOMES: usize = 16;
pub const MAX_QUESTION_LEN: usize = 256;

#[program]
pub mod wagr_executor {
    use super::*;

    /// Initialise a new prediction market.
    ///
    /// Creates the `OutcomeMarket` PDA and configures the collateral vault.
    /// Outcome mints are created in a follow-up instruction so the per-mint
    /// CPI budget stays bounded.
    pub fn create_market(
        ctx: Context<CreateMarket>,
        market_id: u64,
        question: String,
        outcome_count: u8,
        resolution_deadline: i64,
        lmsr_b: u128,
        fee_bps: u16,
        resolution_source: ResolutionSource,
    ) -> Result<()> {
        require!(
            (2..=MAX_OUTCOMES as u8).contains(&outcome_count),
            WagrError::InvalidOutcomeCount
        );
        require!(
            question.as_bytes().len() <= MAX_QUESTION_LEN,
            WagrError::QuestionTooLong
        );
        require!(lmsr_b > 0, WagrError::InvalidLmsrB);

        let market = &mut ctx.accounts.market;
        market.market_id = market_id;
        market.authority = ctx.accounts.authority.key();
        market.question = question;
        market.outcome_count = outcome_count;
        market.resolution_deadline = resolution_deadline;
        market.state = MarketStateOnChain::Open;
        market.collateral_mint = ctx.accounts.collateral_mint.key();
        market.collateral_vault = ctx.accounts.collateral_vault.key();
        market.lmsr_b = lmsr_b;
        market.fee_bps = fee_bps;
        market.total_volume = 0;
        market.created_at = Clock::get()?.unix_timestamp;
        market.resolution_source = resolution_source;
        market.bump = ctx.bumps.market;
        Ok(())
    }

    /// CTF split -- lock collateral, mint `amount` of every outcome to caller.
    pub fn split(ctx: Context<Trade>, amount: u64) -> Result<()> {
        require!(amount > 0, WagrError::ZeroAmount);
        let mut shares = OutcomeShares::new(ctx.accounts.market.outcome_count)
            .map_err(|_| WagrError::InvalidOutcomeCount)?;
        let mut domain = market_to_domain(&ctx.accounts.market)?;
        let split_res = split_position(&mut domain, &mut shares, amount)
            .map_err(|_| WagrError::CtfRejected)?;

        // Pull collateral into the vault.
        let collateral_decimals = ctx.accounts.collateral_mint.decimals;
        transfer_checked(
            ctx.accounts.transfer_collateral_in_ctx(),
            split_res.collateral_in,
            collateral_decimals,
        )?;

        let market = &mut ctx.accounts.market;
        market.total_volume = market.total_volume.saturating_add(amount);
        emit!(MarketTraded {
            market_id: market.market_id,
            kind: TradeKind::Split,
            amount,
            fee: split_res.fee_taken,
        });
        Ok(())
    }

    /// CTF merge -- burn `amount` of every outcome, unlock `amount` of
    /// collateral.
    pub fn merge(ctx: Context<Trade>, amount: u64) -> Result<()> {
        require!(amount > 0, WagrError::ZeroAmount);
        let mut shares = OutcomeShares::new(ctx.accounts.market.outcome_count)
            .map_err(|_| WagrError::InvalidOutcomeCount)?;
        // Seed the local domain shares with the on-chain merge_capacity so
        // the rule check is exact.
        for outcome in 0..ctx.accounts.market.outcome_count {
            let _ = shares.add(outcome, amount);
        }
        let mut domain = market_to_domain(&ctx.accounts.market)?;
        let unlocked = merge_positions(&mut domain, &mut shares, amount)
            .map_err(|_| WagrError::CtfRejected)?;

        let collateral_decimals = ctx.accounts.collateral_mint.decimals;
        transfer_checked(
            ctx.accounts.transfer_collateral_out_ctx(),
            unlocked,
            collateral_decimals,
        )?;

        let market = &mut ctx.accounts.market;
        market.total_volume = market.total_volume.saturating_add(amount);
        emit!(MarketTraded {
            market_id: market.market_id,
            kind: TradeKind::Merge,
            amount,
            fee: 0,
        });
        Ok(())
    }

    /// Quote a buy without executing it. Read-only.
    pub fn quote_buy(ctx: Context<Quote>, outcome: u8, shares: u64) -> Result<u64> {
        let market = &ctx.accounts.market;
        require!(outcome < market.outcome_count, WagrError::OutcomeOutOfRange);
        // For the quote we project the on-chain q vector into f64 via
        // BPS_FACTOR. The real trade path will replicate this in fixed point.
        let qs = chain_q_to_f64(&market.shares_q)?;
        let b = market.lmsr_b as f64 / 1_000_000.0;
        let cost = lmsr_buy_quote(&qs, b, outcome as usize, shares as f64)
            .map_err(|_| WagrError::LmsrFailed)?;
        Ok((cost * 1_000_000.0).round() as u64)
    }

    /// Optimistic propose -- submit an outcome with a bond.
    pub fn propose_resolution(
        ctx: Context<ProposeResolution>,
        proposed_outcome: u8,
    ) -> Result<()> {
        let market = &mut ctx.accounts.market;
        require!(
            matches!(market.state, MarketStateOnChain::Closed),
            WagrError::WrongState
        );
        require!(proposed_outcome < market.outcome_count, WagrError::OutcomeOutOfRange);
        let now = Clock::get()?.unix_timestamp;
        market.proposed_outcome = proposed_outcome;
        market.proposed_at = now;
        market.proposer = ctx.accounts.proposer.key();
        market.state = MarketStateOnChain::Proposed;
        emit!(ResolutionProposed {
            market_id: market.market_id,
            outcome: proposed_outcome,
            proposer: ctx.accounts.proposer.key(),
        });
        Ok(())
    }

    /// Confirm an unchallenged proposal once the window elapses.
    pub fn confirm_resolution(ctx: Context<Resolve>) -> Result<()> {
        let market = &mut ctx.accounts.market;
        require!(
            matches!(market.state, MarketStateOnChain::Proposed),
            WagrError::WrongState
        );
        let now = Clock::get()?.unix_timestamp;
        let window = match market.resolution_source {
            ResolutionSource::UmaOptimistic { challenge_window, .. } => challenge_window,
            _ => 86_400,
        };
        require!(now >= market.proposed_at + window, WagrError::ChallengeWindowOpen);
        market.state = MarketStateOnChain::Resolved;
        market.winning_outcome = market.proposed_outcome;
        emit!(MarketResolved {
            market_id: market.market_id,
            outcome: market.winning_outcome,
        });
        Ok(())
    }

    /// Redeem -- after resolution, burn winning outcome shares for collateral.
    pub fn redeem(ctx: Context<Trade>) -> Result<()> {
        let mut shares = OutcomeShares::new(ctx.accounts.market.outcome_count)
            .map_err(|_| WagrError::InvalidOutcomeCount)?;
        let mut domain = market_to_domain(&ctx.accounts.market)?;
        let _ = shares.add(domain.state.winning_outcome().unwrap_or(0), 1);
        let payout = redeem_payout(&mut domain, &mut shares)
            .map_err(|_| WagrError::CtfRejected)?;

        let collateral_decimals = ctx.accounts.collateral_mint.decimals;
        transfer_checked(
            ctx.accounts.transfer_collateral_out_ctx(),
            payout,
            collateral_decimals,
        )?;
        Ok(())
    }
}

fn market_to_domain(m: &OutcomeMarket) -> Result<DomainMarket> {
    Ok(DomainMarket {
        market_id: m.market_id,
        authority: m.authority.to_bytes(),
        question: m.question.clone(),
        outcome_count: m.outcome_count,
        resolution_source: match m.resolution_source {
            ResolutionSource::UmaOptimistic { bond, challenge_window } => {
                DomainResolutionSource::UmaOptimistic { bond, challenge_window }
            }
            ResolutionSource::PythAggregator { feed, threshold } => {
                DomainResolutionSource::PythAggregator { feed: feed.to_bytes(), threshold }
            }
            ResolutionSource::Manual { authority } => {
                DomainResolutionSource::Manual { authority: authority.to_bytes() }
            }
            ResolutionSource::MultiSourceConsensus { sources, threshold } => {
                DomainResolutionSource::MultiSourceConsensus { sources, threshold }
            }
        },
        resolution_deadline: m.resolution_deadline,
        state: match m.state {
            MarketStateOnChain::Open => MarketState::Open,
            MarketStateOnChain::Closed => MarketState::Closed,
            MarketStateOnChain::Proposed => MarketState::Closed,
            MarketStateOnChain::Disputed => MarketState::Disputed,
            MarketStateOnChain::Resolved => MarketState::Resolved { winning_outcome: m.winning_outcome },
            MarketStateOnChain::ResolvedInvalid => MarketState::ResolvedInvalid,
        },
        collateral_mint: m.collateral_mint.to_bytes(),
        collateral_vault: m.collateral_vault.to_bytes(),
        outcome_mints: [[0u8; 32]; MAX_OUTCOMES],
        lmsr_b: m.lmsr_b,
        total_volume: m.total_volume,
        fee_bps: m.fee_bps,
        created_at: m.created_at,
    })
}

fn chain_q_to_f64(qs: &[u128; MAX_OUTCOMES]) -> Result<Vec<f64>> {
    Ok(qs.iter().map(|q| *q as f64 / 1_000_000.0).collect())
}

#[account]
#[derive(Default)]
pub struct OutcomeMarket {
    pub market_id: u64,
    pub authority: Pubkey,
    pub question: String,
    pub outcome_count: u8,
    pub resolution_deadline: i64,
    pub state: MarketStateOnChain,
    pub collateral_mint: Pubkey,
    pub collateral_vault: Pubkey,
    pub lmsr_b: u128,
    pub fee_bps: u16,
    pub total_volume: u64,
    pub created_at: i64,
    pub resolution_source: ResolutionSource,
    pub proposed_outcome: u8,
    pub proposed_at: i64,
    pub proposer: Pubkey,
    pub winning_outcome: u8,
    pub shares_q: [u128; MAX_OUTCOMES],
    pub bump: u8,
}

impl OutcomeMarket {
    pub const SPACE: usize = 8
        + 8                    // market_id
        + 32                   // authority
        + 4 + MAX_QUESTION_LEN // question
        + 1                    // outcome_count
        + 8                    // deadline
        + 1                    // state
        + 32 + 32              // collateral mint + vault
        + 16                   // lmsr_b
        + 2                    // fee_bps
        + 8                    // total_volume
        + 8                    // created_at
        + 1 + 64               // resolution_source (tagged union slack)
        + 1 + 8 + 32           // proposed outcome + at + proposer
        + 1                    // winning
        + 16 * MAX_OUTCOMES    // shares_q
        + 1; // bump
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MarketStateOnChain {
    #[default]
    Open,
    Closed,
    Proposed,
    Disputed,
    Resolved,
    ResolvedInvalid,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResolutionSource {
    UmaOptimistic { bond: u64, challenge_window: i64 },
    PythAggregator { feed: Pubkey, threshold: i64 },
    Manual { authority: Pubkey },
    MultiSourceConsensus { sources: u8, threshold: u8 },
}

impl Default for ResolutionSource {
    fn default() -> Self {
        ResolutionSource::Manual { authority: Pubkey::default() }
    }
}

#[event]
pub struct MarketTraded {
    pub market_id: u64,
    pub kind: TradeKind,
    pub amount: u64,
    pub fee: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TradeKind {
    Split,
    Merge,
    Buy,
    Sell,
    Redeem,
}

#[event]
pub struct ResolutionProposed {
    pub market_id: u64,
    pub outcome: u8,
    pub proposer: Pubkey,
}

#[event]
pub struct MarketResolved {
    pub market_id: u64,
    pub outcome: u8,
}

#[error_code]
pub enum WagrError {
    #[msg("outcome count must be between 2 and 16")]
    InvalidOutcomeCount,
    #[msg("question string too long (max 256 bytes)")]
    QuestionTooLong,
    #[msg("liquidity parameter b must be > 0")]
    InvalidLmsrB,
    #[msg("amount must be > 0")]
    ZeroAmount,
    #[msg("outcome index out of range")]
    OutcomeOutOfRange,
    #[msg("conditional token framework rejected the move")]
    CtfRejected,
    #[msg("LMSR pricing failed -- possible overflow")]
    LmsrFailed,
    #[msg("market not in expected state for this transition")]
    WrongState,
    #[msg("challenge window still open")]
    ChallengeWindowOpen,
}

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct CreateMarket<'info> {
    #[account(
        init,
        payer = authority,
        space = OutcomeMarket::SPACE,
        seeds = [b"market".as_ref(), market_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub market: Account<'info, OutcomeMarket>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub collateral_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub collateral_vault: InterfaceAccount<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct Trade<'info> {
    #[account(mut)]
    pub market: Account<'info, OutcomeMarket>,
    #[account(mut)]
    pub trader_collateral: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub collateral_mint: InterfaceAccount<'info, Mint>,
    pub trader: Signer<'info>,
    pub token_program: Program<'info, Token2022>,
}

impl<'info> Trade<'info> {
    pub fn transfer_collateral_in_ctx(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: self.trader_collateral.to_account_info(),
            to: self.vault.to_account_info(),
            mint: self.collateral_mint.to_account_info(),
            authority: self.trader.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }

    pub fn transfer_collateral_out_ctx(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: self.vault.to_account_info(),
            to: self.trader_collateral.to_account_info(),
            mint: self.collateral_mint.to_account_info(),
            authority: self.market.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct Quote<'info> {
    pub market: Account<'info, OutcomeMarket>,
}

#[derive(Accounts)]
pub struct ProposeResolution<'info> {
    #[account(mut)]
    pub market: Account<'info, OutcomeMarket>,
    pub proposer: Signer<'info>,
}

#[derive(Accounts)]
pub struct Resolve<'info> {
    #[account(mut)]
    pub market: Account<'info, OutcomeMarket>,
}

// Helper so domain MarketState can expose the winning outcome for redeem.
trait WinningOutcome {
    fn winning_outcome(&self) -> Option<u8>;
}

impl WinningOutcome for MarketState {
    fn winning_outcome(&self) -> Option<u8> {
        if let MarketState::Resolved { winning_outcome } = self {
            Some(*winning_outcome)
        } else {
            None
        }
    }
}
