/**
 * Hanson (2003) Logarithmic Market Scoring Rule.
 *
 * This file is the TypeScript twin of `packages/lmsr-amm/rust/src/lib.rs` --
 * the two implementations are property-tested against each other in
 * `tests/lmsr-parity.test.ts`.
 *
 * Cost function:  C(q) = b · ln( Σ exp(q_i / b) )
 * Price function: p_i  = exp(q_i / b) / Σ exp(q_j / b)
 */

/** Numerically-stable log-sum-exp. */
export function logSumExp(xs: readonly number[]): number {
  if (xs.length === 0) return Number.NEGATIVE_INFINITY;
  let m = Number.NEGATIVE_INFINITY;
  for (const x of xs) if (x > m) m = x;
  if (!Number.isFinite(m)) return m;
  let sum = 0;
  for (const x of xs) sum += Math.exp(x - m);
  return m + Math.log(sum);
}

export function lmsrCost(qs: readonly number[], b: number): number {
  if (b <= 0) throw new RangeError("b must be > 0");
  return b * logSumExp(qs.map((q) => q / b));
}

export function lmsrPrice(
  qs: readonly number[],
  b: number,
  outcome: number,
): number {
  if (outcome < 0 || outcome >= qs.length) throw new RangeError("outcome OOR");
  const scaled = qs.map((q) => q / b);
  let m = Number.NEGATIVE_INFINITY;
  for (const x of scaled) if (x > m) m = x;
  let sum = 0;
  let top = 0;
  for (let i = 0; i < scaled.length; i++) {
    const e = Math.exp(scaled[i] - m);
    sum += e;
    if (i === outcome) top = e;
  }
  return top / sum;
}

export function lmsrBuyQuote(
  qs: readonly number[],
  b: number,
  outcome: number,
  shares: number,
): number {
  if (shares < 0) throw new RangeError("shares must be >= 0");
  const next = qs.slice();
  next[outcome] = (next[outcome] ?? 0) + shares;
  return lmsrCost(next, b) - lmsrCost(qs, b);
}

export function lmsrSellQuote(
  qs: readonly number[],
  b: number,
  outcome: number,
  shares: number,
): number {
  if (shares < 0 || (qs[outcome] ?? 0) < shares)
    throw new RangeError("invalid sell");
  const next = qs.slice();
  next[outcome] = (next[outcome] ?? 0) - shares;
  return lmsrCost(qs, b) - lmsrCost(next, b);
}

/** Worst-case AMM loss for an N-outcome market = b · ln(N). */
export function lmsrMaxLoss(b: number, outcomes: number): number {
  return b * Math.log(outcomes);
}

/** Quick all-outcome price vector -- handy for charts. */
export function lmsrPriceVector(
  qs: readonly number[],
  b: number,
): number[] {
  const scaled = qs.map((q) => q / b);
  let m = Number.NEGATIVE_INFINITY;
  for (const x of scaled) if (x > m) m = x;
  let sum = 0;
  const es: number[] = [];
  for (const x of scaled) {
    const e = Math.exp(x - m);
    sum += e;
    es.push(e);
  }
  return es.map((e) => e / sum);
}
