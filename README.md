<p align="center">
  <img src="./assets/hero.png" alt="WAGR -- Solana Prediction Market Standard. Oracle temple at dusk." width="100%">
</p>

<h1 align="center">WAGR &mdash; Solana Prediction Market Standard</h1>

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-Apache%202.0-D4AF37.svg" alt="License Apache 2.0"></a>
  <a href="#"><img src="https://img.shields.io/badge/build-passing-5BC0EB.svg" alt="Build passing"></a>
  <a href="https://explorer.solana.com/address/GreSDUbtzBpDRgCYo9sXGZbFDM3HXFQWTdeAacF8HDEc?cluster=devnet"><img src="https://img.shields.io/badge/Solana-devnet-FFD93D.svg" alt="Solana devnet"></a>
  <a href="https://www.anchor-lang.com"><img src="https://img.shields.io/badge/Anchor-0.31-F5F2E8.svg" alt="Anchor 0.31"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-1.95-3D2E5C.svg" alt="Rust 1.95"></a>
  <a href="https://www.typescriptlang.org"><img src="https://img.shields.io/badge/TypeScript-5.7-5BC0EB.svg" alt="TypeScript 5.7"></a>
  <a href="#documentation"><img src="https://img.shields.io/badge/docs-online-D4AF37.svg" alt="Docs"></a>
  <a href="https://wagr.fi"><img src="https://img.shields.io/badge/site-wagr.fi-FFD93D.svg" alt="Site wagr.fi"></a>
  <a href="https://x.com/wagrfi"><img src="https://img.shields.io/badge/X-@wagrfi-F5F2E8.svg" alt="X @wagrfi"></a>
  <a href="https://www.npmjs.com/package/wagr-cli"><img src="https://img.shields.io/npm/v/wagr-cli?label=wagr-cli&color=D4AF37" alt="npm wagr-cli"></a>
  <a href="https://www.npmjs.com/package/@wagrlabs/sdk"><img src="https://img.shields.io/npm/v/%40wagrlabs%2Fsdk?label=%40wagrlabs%2Fsdk&color=FF8C00" alt="npm @wagrlabs/sdk"></a>
</p>

<p align="center"><em>CA: Eb2zbWj8a9oes8t4DC1kjuQEPWgjYCgjGSXqLoLpump</em></p>

<p align="center"><em>Bet the truth.</em></p>

<p align="center">Solana's first prediction market standard. One Anchor program, five outcome modules, three client surfaces.</p>

## Why WAGR

Polymarket is Ethereum's prediction market. Solana has Drift Prediction -- one dApp -- and no shared standard. WAGR is the missing rail: Outcome Token, Conditional Token Framework, Optimistic Oracle, LMSR AMM, and Multi-Outcome markets, all as composable modules over a single Anchor program.

```mermaid
%%{init: {'theme':'base', 'themeVariables': {
  'primaryColor':'#F5F2E8',
  'primaryTextColor':'#3D2E5C',
  'primaryBorderColor':'#D4AF37',
  'lineColor':'#FFD93D',
  'tertiaryColor':'#3D2E5C'
}}}%%
flowchart LR
    SDK[sdk-ts] --> ANCHOR[Anchor executor]
    CLI[wagr-cli] --> SDK
    WEB[Web] --> SDK
    MOB[Mobile] --> SDK
    TG[Telegram bot] --> SDK
    ANCHOR --> TOK[Outcome Token-2022]
    ANCHOR --> CTF[CTF]
    ANCHOR --> LMSR[LMSR AMM]
    ANCHOR --> ORC[Oracle Resolver]
    ORC --> UMA[UMA bridge]
    ORC --> PYTH[Pyth aggregator]
    ANCHOR --> REP[Reputation DVM]
```

## Documentation

- [Architecture](./docs/architecture.md)
- [Outcome specification](./docs/outcome-spec.md) -- Outcome Token + CTF + LMSR
- [Oracle specification](./docs/oracle-spec.md) -- UMA optimistic, Pyth, multi-source
- [Security model](./docs/security.md) -- bonds, slashing, vault PDAs

## Modules

| Crate | Purpose |
|---|---|
| [`outcome-engine`](./src/outcome-engine) | Outcome Token + Conditional Token Framework |
| [`lmsr-amm`](./src/lmsr-amm) | Hanson 2003 LMSR pricing |
| [`oracle-resolver`](./src/oracle-resolver) | UMA-style optimistic + Pyth + consensus |
| [`reputation-module`](./src/reputation-module) | Dispute DVM with $WAGR slashing |
| [`anchor-program`](./src/anchor-program) | Solana executor (devnet; mainnet pending) |

## Quick Tour

```bash
# Trade-side
cargo test --workspace
pnpm --filter @wagrlabs/sdk build

# CLI
npm i -g wagr-cli
npm i @wagrlabs/sdk
wagr quote --qs 0,0 --b 1000 --outcome 0 --shares 100
wagr split --market 1 --amount 100
wagr propose --market 1 --outcome 0 --bond 1000000
```

## Clients & Status

Live surfaces:

| Surface | State | Where |
|---|---|---|
| Web (landing + Designer + Markets + Docs) | live | https://wagr.fi |
| `wagr-cli` | live on npm | `npm i -g wagr-cli` &mdash; https://www.npmjs.com/package/wagr-cli |
| `@wagrlabs/sdk` | live on npm | `npm i @wagrlabs/sdk` &mdash; https://www.npmjs.com/package/@wagrlabs/sdk |
| Anchor program | live on **Solana devnet** (mainnet pending) | [`GreSDUbtzBpDRgCYo9sXGZbFDM3HXFQWTdeAacF8HDEc`](https://explorer.solana.com/address/GreSDUbtzBpDRgCYo9sXGZbFDM3HXFQWTdeAacF8HDEc?cluster=devnet) |

In development:

| Surface | State |
|---|---|
| Mobile app (`packages/mobile-app`, Solana Mobile SDK) | in development, not published |
| Telegram bot (`packages/telegram-bot`) | in development, not yet running on a public bot |

The architecture diagram above draws Mobile and Telegram as clients because the SDK is wired for them; the surfaces themselves are not yet shipped. The CLI's `create` command is an honest **dry-run preview** today (it prints the instruction body and the chosen accounts; it does not submit on chain).

## Reading List

Designed by reading -- and citing -- the foundational papers in prediction markets:

- Hanson, R. (2003). "Combinatorial Information Market Design." *Information Systems Frontiers*.
- UMA Project (2020). *Optimistic Oracle: A trust-minimised data feed for smart contracts.*
- Gnosis (2019). *Conditional Token Framework.*
- Pyth Network (2021). *Pyth: A High-Frequency Oracle for Solana.*
- Augur v2 (2020). *Decentralised Oracle Protocol.*

## Repository Layout

```
src/
├── outcome-engine/        Rust core -- Outcome Token + CTF
├── lmsr-amm/              Hanson 2003 LMSR + CPMM hybrid
├── oracle-resolver/       UMA optimistic + Pyth + consensus
├── reputation-module/     Dispute DVM + slashing
├── anchor-program/        Solana executor (devnet; mainnet pending)
└── sdk/                   TypeScript developer SDK
```

## License

Apache 2.0 -- see [LICENSE](./LICENSE).
