# WAGR Architecture

> Solana's first prediction market standard. One Anchor program, five outcome modules, three client surfaces.

## 1. Top-Level Topology

```mermaid
flowchart LR
    classDef client fill:#5BC0EB,stroke:#1a0d2e,color:#1a0d2e
    classDef sdk fill:#D4AF37,stroke:#1a0d2e,color:#1a0d2e
    classDef onchain fill:#F5F2E8,stroke:#3D2E5C,color:#3D2E5C
    classDef ext fill:#FFD93D,stroke:#3D2E5C,color:#3D2E5C
    classDef svc fill:#3D2E5C,stroke:#D4AF37,color:#F5F2E8

    subgraph Clients
        WEB[apps/web -- Greek Temple 3D]:::client
        MKT[apps/markets -- /markets dashboard]:::client
        MOB[mobile-app -- WAGR Live]:::client
        CLI[wagr-cli]:::client
        TG[telegram-bot]:::client
    end

    subgraph SDK
        SDKTS[sdk-ts]:::sdk
    end

    subgraph Service
        SVC[FastAPI metadata + LMSR]:::svc
        DB[(Postgres)]:::svc
        REDIS[(Redis)]:::svc
    end

    subgraph Solana
        ANCHOR[wagr-executor program]:::onchain
        TOK[Outcome Token mints -- Token-2022]:::onchain
        VAULT[Collateral vault]:::onchain
    end

    subgraph External
        HELIUS[Helius RPC]:::ext
        PYTH[Pyth aggregator]:::ext
        UMA[UMA OO bridge]:::ext
    end

    WEB --> SDKTS
    MKT --> SDKTS
    MOB --> SDKTS
    CLI --> SDKTS
    TG --> SDKTS

    SDKTS --> ANCHOR
    WEB --> SVC
    MKT --> SVC
    MOB --> SVC
    TG --> SVC

    SVC --> DB
    SVC --> REDIS
    SVC --> HELIUS

    ANCHOR --> TOK
    ANCHOR --> VAULT
    ANCHOR <--> PYTH
    ANCHOR <--> UMA
```

## 2. Module Map

| Module | Crate / Package | Role |
|---|---|---|
| Outcome Token Standard | `wagr-outcome-engine` | Per-outcome share accounting, Token-2022 binding |
| Conditional Token Framework | `wagr-outcome-engine::ctf` | Split / merge / redeem |
| LMSR AMM | `wagr-lmsr-amm` (+ TS twin) | Hanson 2003 pricing |
| Oracle Resolver | `wagr-oracle-resolver` | UMA optimistic + multi-source consensus |
| Reputation Module | `wagr-reputation-module` | DVM tally + $WAGR slashing |
| Anchor Executor | `wagr-executor` | On-chain glue; thin program |
| SDK | `sdk-ts` | Public client surface |
| Designer | `apps/web/designer` | 1st hook: author + backtest |
| Dashboard | `apps/web/markets` | 2nd hook: live trading |
| Mobile + Bot + CLI | `mobile-app`, `telegram-bot`, `cli` | 3rd hook |

## 3. Trade Lifecycle

```mermaid
sequenceDiagram
    participant T as Trader
    participant W as Wallet
    participant S as SDK
    participant P as Anchor program
    participant V as Vault
    participant M as Mint (per outcome)

    T->>S: split(market, amount)
    S->>P: composeTx(splitIx)
    P->>V: transferChecked(collateral, amount)
    loop for each outcome
        P->>M: mintTo(trader, amount)
    end
    P-->>S: signature
    S-->>T: TradeResult
```

## 4. Resolution Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Open
    Open --> Closed: deadline passed
    Closed --> Proposed: propose(outcome, bond)
    Proposed --> Confirmed: window elapsed
    Proposed --> Disputed: challenge(bond)
    Disputed --> Settled: DVM verdict
    Confirmed --> [*]
    Settled --> [*]
```

## 5. Why Not Use the Existing UMA Bridge?

UMA's mainnet bridge to Solana is an event log without first-class dispute primitives on the Solana side. WAGR ports the **state machine** -- `Proposed → Challenged → Settled` -- so the dispute window, bond accounting, and slashing all live in a single PDA. The bridge is still consulted for the canonical DVM verdict; we just refuse to let traders touch the locked collateral until the local state confirms.

## 6. Why LMSR, Not pure CPMM?

For thinly traded markets a constant-product invariant gives terrible price discovery (the first $10 of YES quote moves the price 20 points). LMSR bounds the AMM's worst-case loss at `b · ln(N)` regardless of volume, which is the right shape during bootstrap. Once a market crosses a configurable depth threshold we switch to CPMM to save gas.

## 7. Security Boundary

- Trader wallets sign every state-changing instruction.
- The vault is owned by a PDA derived from `(b"market", market_id)`; the program is the only account that can transfer collateral out.
- `HELIUS_RPC_URL` lives **only** in `service/` and `apps/web/app/api/das/route.ts`. The wallet adapter uses a public RPC.
- The dispute path is funded entirely by bonds; the protocol takes no custody of voter capital outside the slash window.
