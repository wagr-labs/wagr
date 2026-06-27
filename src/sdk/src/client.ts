import BN from "bn.js";
// @coral-xyz/anchor 0.31 is CommonJS: ESM named value imports crash at runtime
// (cjs-module-lexer can't see the named exports) even though tsc passes. Pull
// the namespace via the default import and reach the runtime constructors as
// members off it (anchor.AnchorProvider / anchor.Program). Types are erased, so
// they stay as a pure `import type` and never hit the runtime named-import path.
import anchor from "@coral-xyz/anchor";
import type { AnchorProvider, Idl, Program, Wallet } from "@coral-xyz/anchor";
import {
  Connection,
  PublicKey,
  type ConfirmOptions,
  type Commitment,
} from "@solana/web3.js";
import { WAGR_PROGRAM_ID, TOKEN_2022_PROGRAM_ID } from "./constants.js";
import type { OutcomeMarketAccount, TradeResult } from "./types.js";

export interface WagrClientOptions {
  connection: Connection;
  wallet: Wallet;
  programId?: PublicKey;
  commitment?: Commitment;
}

const COMMITMENT_DEFAULT: Commitment = "confirmed";

/**
 * Read / write client for the WAGR outcome executor.
 *
 * The class is intentionally narrow: high-level helpers (e.g. "buy YES at
 * market") layer on top via `MarketHelpers`. This keeps the bundle size
 * dApp-friendly for the wallet-adapter path.
 */
export class WagrClient {
  readonly program: Program<Idl>;
  readonly provider: AnchorProvider;
  readonly programId: PublicKey;

  constructor(opts: WagrClientOptions, idl?: Idl) {
    const commitOptions: ConfirmOptions = {
      commitment: opts.commitment ?? COMMITMENT_DEFAULT,
      preflightCommitment: opts.commitment ?? COMMITMENT_DEFAULT,
    };
    this.provider = new anchor.AnchorProvider(
      opts.connection,
      opts.wallet,
      commitOptions,
    );
    this.programId = opts.programId ?? WAGR_PROGRAM_ID;
    if (!idl) {
      // The IDL is loaded lazily via `Program.fetchIdl` when callers want it.
      // For UI flows that already shipped the IDL, pass it in directly.
      this.program = undefined as unknown as Program<Idl>;
    } else {
      this.program = new anchor.Program(idl, this.provider);
    }
  }

  /** Compute the PDA for a market_id. */
  marketAddress(marketId: bigint | BN): PublicKey {
    const id = marketId instanceof BN ? marketId : new BN(marketId.toString());
    const buf = id.toArrayLike(Buffer, "le", 8);
    return PublicKey.findProgramAddressSync(
      [Buffer.from("market"), buf],
      this.programId,
    )[0];
  }

  /** Fetch and decode an `OutcomeMarket` account. */
  async fetchMarket(marketId: bigint | BN): Promise<OutcomeMarketAccount> {
    if (!this.program) throw new Error("IDL not loaded -- pass it to ctor");
    const pda = this.marketAddress(marketId);
    const raw = await (
      this.program.account as unknown as {
        outcomeMarket: { fetch: (k: PublicKey) => Promise<unknown> };
      }
    ).outcomeMarket.fetch(pda);
    return raw as OutcomeMarketAccount;
  }

  /** Build an unsigned `split` transaction. Caller signs + sends. */
  async splitIx(marketId: bigint | BN, amount: bigint) {
    if (!this.program) throw new Error("IDL not loaded -- pass it to ctor");
    const pda = this.marketAddress(marketId);
    return this.program.methods
      .split(new BN(amount.toString()))
      .accountsPartial({
        market: pda,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();
  }

  async mergeIx(marketId: bigint | BN, amount: bigint) {
    if (!this.program) throw new Error("IDL not loaded -- pass it to ctor");
    const pda = this.marketAddress(marketId);
    return this.program.methods
      .merge(new BN(amount.toString()))
      .accountsPartial({
        market: pda,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();
  }

  /** Helper: end-to-end `split` that picks ATAs and submits. */
  async split(
    marketId: bigint | BN,
    amount: bigint,
  ): Promise<TradeResult> {
    const ix = await this.splitIx(marketId, amount);
    const sig = await this.provider.sendAndConfirm(
      new (await import("@solana/web3.js")).Transaction().add(ix),
    );
    return {
      signature: sig,
      collateralIn: amount,
      sharesOut: amount,
      fee: 0n,
    };
  }
}
