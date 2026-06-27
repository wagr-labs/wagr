import type { PublicKey } from "@solana/web3.js";
import type BN from "bn.js";

export type MarketState =
  | "Open"
  | "Closed"
  | "Proposed"
  | "Disputed"
  | "Resolved"
  | "ResolvedInvalid";

export type ResolutionSource =
  | { kind: "UmaOptimistic"; bond: BN; challengeWindow: BN }
  | { kind: "PythAggregator"; feed: PublicKey; threshold: BN }
  | { kind: "Manual"; authority: PublicKey }
  | { kind: "MultiSourceConsensus"; sources: number; threshold: number };

/** Mirrors the Anchor `OutcomeMarket` account layout. */
export interface OutcomeMarketAccount {
  marketId: BN;
  authority: PublicKey;
  question: string;
  outcomeCount: number;
  resolutionDeadline: BN;
  state: MarketState;
  collateralMint: PublicKey;
  collateralVault: PublicKey;
  lmsrB: BN;
  feeBps: number;
  totalVolume: BN;
  createdAt: BN;
  resolutionSource: ResolutionSource;
  proposedOutcome: number;
  proposedAt: BN;
  proposer: PublicKey;
  winningOutcome: number;
  sharesQ: BN[];
  bump: number;
}

/** Display-friendly market summary returned from the metadata service. */
export interface MarketSummary {
  marketId: string;
  question: string;
  outcomeCount: number;
  outcomes: { label: string; price: number; volume: number }[];
  state: MarketState;
  resolutionDeadline: string;
  collateralSymbol: string;
  totalVolume: number;
}

export interface TradeResult {
  signature: string;
  collateralIn: bigint;
  sharesOut: bigint;
  fee: bigint;
}
