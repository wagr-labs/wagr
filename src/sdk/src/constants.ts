import { PublicKey } from "@solana/web3.js";

/** Anchor program ID for the WAGR outcome executor on Solana mainnet. */
export const WAGR_PROGRAM_ID = new PublicKey(
  "GreSDUbtzBpDRgCYo9sXGZbFDM3HXFQWTdeAacF8HDEc",
);

/** Token-2022 program ID -- collateral and outcome shares live here. */
export const TOKEN_2022_PROGRAM_ID = new PublicKey(
  "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
);

/** Mainnet USDC mint (default collateral). */
export const USDC_MINT_MAINNET = new PublicKey(
  "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
);

/** Six-decimal fixed-point scale shared with the Rust core. */
export const BPS_FACTOR = 1_000_000n;

export const MAX_OUTCOMES = 16;

/** Public RPC default (safe to expose to wallet adapter). */
export const DEFAULT_PUBLIC_RPC =
  "https://api.mainnet-beta.solana.com" as const;
