/** One investor contribution in a batch funding call. */
export type FundingBatchEntry = [investor: string, amount: string];

export interface FundingBatchRetryState {
  /** Each investor's contribution immediately before submitting the batch. */
  before: ReadonlyMap<string, string>;
  /** Each investor's contribution read from the contract after the failure. */
  current: ReadonlyMap<string, string>;
}

/**
 * Reconcile an atomic funding batch after an ambiguous RPC failure.
 *
 * Returns the complete batch if none of its changes are present, or an empty
 * array if the complete batch was applied. Throws if current state is
 * inconsistent with either outcome; partial retries are not safe for an
 * atomic batch.
 */
export function filterFundingBatchForRetry(
  entries: readonly (readonly [investor: string, amount: string])[],
  state: FundingBatchRetryState,
): Array<[investor: string, amount: string]> {
  if (entries.length === 0) {
    throw new RangeError("Funding batch must not be empty");
  }

  const amountsByInvestor = new Map<string, bigint>();
  for (const [investor, amount] of entries) {
    if (investor.trim() === "") {
      throw new TypeError("Funding batch investor must not be empty");
    }

    const parsedAmount = parseNonNegativeInteger(amount, "batch amount");
    if (parsedAmount === 0n) {
      throw new RangeError("Funding batch amounts must be positive");
    }
    amountsByInvestor.set(
      investor,
      (amountsByInvestor.get(investor) ?? 0n) + parsedAmount,
    );
  }

  let appliedCount = 0;
  for (const [investor, amount] of amountsByInvestor) {
    const beforeValue = state.before.get(investor);
    const currentValue = state.current.get(investor);
    if (beforeValue === undefined || currentValue === undefined) {
      throw new Error(`Missing contribution state for investor ${investor}`);
    }

    const before = parseNonNegativeInteger(beforeValue, "pre-batch contribution");
    const current = parseNonNegativeInteger(currentValue, "current contribution");
    if (current === before) {
      continue;
    }
    if (current === before + amount) {
      appliedCount += 1;
      continue;
    }
    throw new Error(
      `Contribution state for investor ${investor} matches neither the pre-batch nor applied value`,
    );
  }

  if (appliedCount === 0) {
    return entries.map(
      ([investor, amount]): [string, string] => [investor, amount],
    );
  }
  if (appliedCount === amountsByInvestor.size) {
    return [];
  }
  throw new Error(
    "Funding batch state is mixed; refusing to retry only part of an atomic batch",
  );
}

function parseNonNegativeInteger(value: string, label: string): bigint {
  if (!/^\d+$/.test(value)) {
    throw new TypeError(`${label} must be a non-negative integer string`);
  }
  return BigInt(value);
}
