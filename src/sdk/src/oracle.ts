/**
 * UMA-style Optimistic Oracle state machine, TS twin of the Rust
 * `wagr-oracle-resolver` crate. Bond movement and slashing are enforced on
 * chain; this module powers UI-side simulation and the CLI's `wagr dispute`
 * dry-run.
 */

export type ProposalState =
  | "Proposed"
  | "Challenged"
  | "Confirmed"
  | "Settled";

export interface OptimisticProposal {
  marketId: bigint;
  round: number;
  proposer: string;
  bond: bigint;
  proposedOutcome: number;
  proposedAt: number;
  challengeWindow: number;
  state: ProposalState;
  challenger?: string;
  challengerBond?: bigint;
}

export function challenge(
  p: OptimisticProposal,
  challenger: string,
  bond: bigint,
): OptimisticProposal {
  if (p.state !== "Proposed") throw new Error("invalid transition");
  if (p.challenger) throw new Error("already challenged");
  return {
    ...p,
    state: "Challenged",
    challenger,
    challengerBond: bond,
  };
}

export function confirm(
  p: OptimisticProposal,
  now: number,
): { proposal: OptimisticProposal; outcome: number } {
  if (p.state !== "Proposed") throw new Error("invalid transition");
  if (now < p.proposedAt + p.challengeWindow)
    throw new Error("challenge window still open");
  return {
    proposal: { ...p, state: "Confirmed" },
    outcome: p.proposedOutcome,
  };
}

export interface FeedSample {
  source: string;
  value: bigint;
  confidence: bigint;
  timestamp: number;
}

export class MultiSourceConsensus {
  readonly threshold: number;
  readonly maxStaleness: number;
  private samples: FeedSample[] = [];

  constructor(threshold: number, maxStaleness: number) {
    this.threshold = threshold;
    this.maxStaleness = maxStaleness;
  }

  ingest(sample: FeedSample) {
    this.samples = this.samples.filter((s) => s.source !== sample.source);
    this.samples.push(sample);
  }

  aggregate(now: number): { median: bigint; confirms: number } {
    const fresh = this.samples
      .filter((s) => now - s.timestamp <= this.maxStaleness)
      .map((s) => s.value);
    if (fresh.length < this.threshold) {
      throw new Error(
        `no consensus -- ${fresh.length}/${this.threshold} confirmations`,
      );
    }
    fresh.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    const mid = Math.floor(fresh.length / 2);
    const median =
      fresh.length % 2 === 0
        ? (fresh[mid - 1]! + fresh[mid]!) / 2n
        : fresh[mid]!;
    return { median, confirms: fresh.length };
  }

  resolveBinary(now: number, threshold: bigint): number {
    const { median } = this.aggregate(now);
    return median >= threshold ? 0 : 1;
  }
}
