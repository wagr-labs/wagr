/**
 * WAGR TypeScript SDK
 *
 * Public surface for developers building on the WAGR Prediction Market
 * Standard. Three pillars:
 *
 *  - `WagrClient`     -- read / write to the Anchor program on Solana.
 *  - `lmsr`           -- Hanson 2003 LMSR pricing helpers (matches the Rust
 *                        implementation byte-for-byte at the test boundary).
 *  - `oracle`         -- UMA-style optimistic resolution state machine.
 *
 * The SDK is RPC-agnostic. Wallet adapters from `@solana/wallet-adapter-base`
 * or the Solana Mobile SDK can be passed in unchanged.
 */

export * from "./client.js";
export * from "./types.js";
export * as lmsr from "./lmsr.js";
export * as oracle from "./oracle.js";
export { WAGR_PROGRAM_ID } from "./constants.js";
