// karis-ky Escrow SDK — Type Definitions
//
// Auto-generated from contract spec (spec.json) and audited against the
// Rust source (escrow/src/lib.rs). Schema version 6, interface version 1.
//
// See docs/escrow-error-messages.md for the full error code reference.

// ---------------------------------------------------------------------------
// Status enum
// ---------------------------------------------------------------------------

/** Escrow lifecycle status. Forward-only transitions. */
export enum EscrowStatus {
  Open = 0,
  Funded = 1,
  Settled = 2,
  Withdrawn = 3,
  Cancelled = 4,
}

/** Human-readable labels for each status value. */
export const ESCROW_STATUS_LABELS: Record<EscrowStatus, string> = {
  [EscrowStatus.Open]: "Open",
  [EscrowStatus.Funded]: "Funded",
  [EscrowStatus.Settled]: "Settled",
  [EscrowStatus.Withdrawn]: "Withdrawn",
  [EscrowStatus.Cancelled]: "Cancelled",
};

// ---------------------------------------------------------------------------
// Core data types (mirrors contracttype structs in lib.rs)
// ---------------------------------------------------------------------------

/** Full escrow snapshot persisted at DataKey::Escrow. */
export interface InvoiceEscrow {
  invoice_id: string;
  admin: string;
  sme_address: string;
  amount: string; // i128 → bigint string
  funding_target: string;
  funded_amount: string;
  yield_bps: string; // i64 → bigint string
  maturity: string; // u64 → bigint string
  status: EscrowStatus;
}

/** One step in the optional tiered yield ladder. Immutable after init. */
export interface YieldTier {
  min_lock_secs: string; // u64 → bigint string
  yield_bps: string; // i64 → bigint string
}

/** Captured once when escrow first becomes funded. Immutable pro-rata denominator. */
export interface FundingCloseSnapshot {
  total_principal: string;
  funding_target: string;
  closed_at_ledger_timestamp: string;
  closed_at_ledger_sequence: number; // u32
}

/** SME-reported collateral metadata — record-only, not proof of custody. */
export interface SmeCollateralCommitment {
  asset: string;
  amount: string;
  recorded_at: string;
}

/** Composite escrow summary returned by get_escrow_summary. */
export interface EscrowSummary {
  escrow: InvoiceEscrow;
  has_maturity_lock: boolean;
  legal_hold: boolean;
  funding_close_snapshot: FundingCloseSnapshot | null;
  unique_funder_count: number;
  is_allowlist_active: boolean;
  schema_version: number;
  sme_collateral_commitment: SmeCollateralCommitment | null;
  has_primary_attestation: boolean;
  attestation_log_length: number;
}

/** Structured error diagnostic emitted alongside contract errors. */
export interface ErrorDiagnostic {
  error_code: number;
  message: string;
  recovery_action: string;
  context: string | null;
}

/** Pre-configured escrow template. */
export interface EscrowTemplate {
  yield_bps: string;
  maturity: string;
  yield_tiers: YieldTier[] | null;
  min_contribution: string | null;
  max_unique_investors: number | null;
  funding_deadline_days: number | null;
}

// ---------------------------------------------------------------------------
// Event streaming types
// ---------------------------------------------------------------------------

/** A contract event returned by Soroban RPC, with the escrow contract ID attached. */
export interface EscrowEvent {
  id: string;
  type: string;
  contract_id: string;
  ledger: number;
  ledger_closed_at: string;
  paging_token: string;
  topics: unknown[];
  value: unknown;
  /** Decoded event name when the RPC adapter can provide it. */
  name?: string;
}

/** Result page returned by a Soroban RPC getEvents adapter. */
export interface SorobanEventPage {
  events: EscrowEvent[];
  latest_ledger?: number;
  cursor?: string;
}

/** Controls polling, paging, and cancellation for an escrow event stream. */
export interface EscrowEventSubscriptionOptions {
  /** First ledger to query. Defaults to the current ledger. */
  start_ledger?: number;
  /** Resume from a Soroban paging token. */
  cursor?: string;
  /** Optional decoded event names to deliver. */
  event_names?: readonly string[];
  /** Delay between empty polls. Defaults to 5 seconds. */
  poll_interval_ms?: number;
  /** Maximum events requested per RPC call. Defaults to 100. */
  limit?: number;
  /** Stops the stream without throwing when aborted. */
  signal?: AbortSignal;
}

// ---------------------------------------------------------------------------
// Contract constants
// ---------------------------------------------------------------------------

export const SCHEMA_VERSION = 6;
export const CONTRACT_INTERFACE_VERSION = 1;
export const MAX_INVOICE_ID_STRING_LEN = 32;
export const MAX_ATTESTATION_APPEND_ENTRIES = 32;
export const MAX_INVESTOR_ALLOWLIST_BATCH = 32;
export const MAX_FUND_BATCH = 50;
export const MAX_DUST_SWEEP_AMOUNT = "100000000";

// ---------------------------------------------------------------------------
// Typed error codes (mirrors EscrowError in lib.rs)
// ---------------------------------------------------------------------------

/** Stable typed error codes. Codes are append-only; never reuse or renumber. */
export enum EscrowErrorCode {
  // Init / pricing (1–13)
  AmountMustBePositive = 1,
  YieldBpsOutOfRange = 2,
  EscrowAlreadyInitialized = 3,
  InvoiceIdInvalidLength = 4,
  InvoiceIdInvalidCharset = 5,
  MinContributionNotPositive = 6,
  MinContributionExceedsAmount = 7,
  MaxUniqueInvestorsNotPositive = 8,
  MaxPerInvestorNotPositive = 9,
  TierYieldOutOfRange = 10,
  TierYieldBelowBase = 11,
  TierLockNotIncreasing = 12,
  TierYieldNotNonDecreasing = 13,

  // Uninitialized metadata (20–22)
  EscrowNotInitialized = 20,
  FundingTokenNotSet = 21,
  TreasuryNotSet = 22,

  // Dust sweep + SEP-41 safety (30–42)
  LegalHoldBlocksTreasuryDustSweep = 30,
  SweepAmountNotPositive = 31,
  SweepAmountExceedsMax = 32,
  DustSweepNotTerminal = 33,
  NoFundingTokenBalanceToSweep = 34,
  EffectiveSweepAmountZero = 35,
  TransferAmountNotPositive = 36,
  InsufficientTokenBalanceBeforeTransfer = 37,
  SenderBalanceUnderflow = 38,
  RecipientBalanceUnderflow = 39,
  SenderBalanceDeltaMismatch = 40,
  RecipientBalanceDeltaMismatch = 41,
  SweepExceedsLiabilityFloor = 42,

  // Attestation (50–51)
  PrimaryAttestationAlreadyBound = 50,
  AttestationAppendLogCapacityReached = 51,

  // SME collateral (60–62)
  CollateralAmountNotPositive = 60,
  CollateralAssetEmpty = 61,
  CollateralTimestampBackwards = 62,

  // Admin validation (70–83)
  InvestorBatchEmpty = 70,
  InvestorBatchTooLarge = 71,
  TargetNotPositive = 72,
  TargetUpdateNotOpen = 73,
  TargetBelowFundedAmount = 74,
  CapLowerNotOpen = 75,
  NoInvestorCapConfigured = 76,
  NewCapNotLower = 77,
  NewCapBelowCurrentFunderCount = 78,
  MaturityUpdateNotOpen = 79,
  NewAdminSameAsCurrent = 80,
  FundingBatchEmpty = 82,
  FundingBatchTooLarge = 83,

  // Schema migration (90–92)
  MigrationVersionMismatch = 90,
  AlreadyCurrentSchemaVersion = 91,
  NoMigrationPath = 92,

  // Funding (100–111)
  FundingAmountNotPositive = 100,
  FundingBelowMinContribution = 101,
  LegalHoldBlocksFunding = 102,
  EscrowNotOpenForFunding = 103,
  InvestorNotAllowlisted = 104,
  InvestorContributionOverflow = 105,
  InvestorContributionExceedsCap = 106,
  UniqueInvestorCapReached = 107,
  TieredSecondDeposit = 108,
  InvestorClaimTimeOverflow = 109,
  FundedAmountOverflow = 110,
  CommitmentLockExceedsMaturity = 111,

  // Settlement / payout (120–129)
  LegalHoldBlocksSettlement = 120,
  SettlementNotFunded = 121,
  MaturityNotReached = 122,
  LegalHoldBlocksWithdrawal = 123,
  WithdrawalNotFunded = 124,
  LegalHoldBlocksInvestorClaims = 125,
  NoContributionToClaim = 126,
  InvestorClaimNotSettled = 127,
  InvestorCommitmentLockNotExpired = 128,
  ComputePayoutArithmeticOverflow = 129,

  // Cancel / refund (140–143)
  LegalHoldBlocksCancelFunding = 140,
  CancelFundingNotOpen = 141,
  RefundNotCancelled = 142,
  NoContributionToRefund = 143,

  // Legal-hold clear (150–152)
  LegalHoldClearRequestMissing = 150,
  LegalHoldClearNotReady = 151,
  LegalHoldClearDelayOverflow = 152,

  // Beneficiary rotation + admin handover + funding deadline (160–164)
  LegalHoldBlocksBeneficiaryRotation = 160,
  RotationNotOpen = 161,
  NewSmeSameAsCurrent = 162,
  NoPendingAdmin = 163,
  FundingDeadlinePassed = 164,
}

/** Human-readable error descriptions keyed by error code. */
export const ESCROW_ERROR_LABELS: Record<number, string> = {
  1: "Amount must be positive",
  2: "Yield BPS must be between 0 and 10,000",
  3: "Escrow already initialized",
  4: "Invoice ID length must be 1..=32",
  5: "Invoice ID must contain only [A-Za-z0-9_]",
  6: "Minimum contribution must be positive when configured",
  7: "Minimum contribution cannot exceed invoice amount",
  8: "Max unique investors must be positive when configured",
  9: "Max per investor must be positive when configured",
  10: "Tier yield BPS must be 0..=10,000",
  11: "Tier yield BPS must be >= base yield BPS",
  12: "Tiers must have strictly increasing min_lock_secs",
  13: "Tiers must have non-decreasing yield_bps",
  20: "Escrow not initialized",
  21: "Funding token not set",
  22: "Treasury not set",
  30: "Legal hold blocks treasury dust sweep",
  31: "Sweep amount must be positive",
  32: "Sweep amount exceeds max",
  33: "Dust sweep only in terminal states",
  34: "No funding token balance to sweep",
  35: "Effective sweep amount is zero",
  36: "Transfer amount must be positive",
  37: "Insufficient token balance before transfer",
  38: "Balance underflow on sender",
  39: "Balance underflow on recipient",
  40: "Sender balance delta mismatch",
  41: "Recipient balance delta mismatch",
  42: "Sweep would exceed liability floor",
  50: "Primary attestation already bound",
  51: "Attestation append log capacity reached",
  60: "Collateral amount must be positive",
  61: "Collateral asset symbol must not be empty",
  62: "Collateral timestamp must not go backward",
  70: "Investor batch must be non-empty",
  71: "Investor batch too large",
  72: "Target must be strictly positive",
  73: "Target can only be updated in Open state",
  74: "Target cannot be less than already funded amount",
  75: "Cap can only be lowered in Open state",
  76: "No investor cap configured",
  77: "New cap must be strictly lower than current cap",
  78: "New cap cannot be below current unique funder count",
  79: "Maturity can only be updated in Open state",
  80: "New admin must differ from current admin",
  82: "Funding batch must be non-empty",
  83: "Funding batch too large",
  90: "Migration version mismatch",
  91: "Already at current schema version",
  92: "No migration path",
  100: "Funding amount must be positive",
  101: "Funding amount below min contribution floor",
  102: "Legal hold blocks new funding",
  103: "Escrow not open for funding",
  104: "Investor not on allowlist",
  105: "Investor contribution overflow",
  106: "Investor contribution exceeds per-investor cap",
  107: "Unique investor cap reached",
  108: "Additional principal after tiered first deposit must use fund()",
  109: "Investor claim time overflow",
  110: "Funded amount overflow",
  111: "Commitment lock exceeds escrow maturity",
  120: "Legal hold blocks settlement",
  121: "Escrow must be funded before settlement",
  122: "Escrow has not yet reached maturity",
  123: "Legal hold blocks SME withdrawal",
  124: "Escrow must be funded before withdrawal",
  125: "Legal hold blocks investor claims",
  126: "Address has no contribution to claim",
  127: "Escrow must be settled before investor claim",
  128: "Investor commitment lock not expired",
  129: "Payout arithmetic overflow",
  140: "Legal hold blocks cancel funding",
  141: "Cancel funding only allowed in Open state",
  142: "Refund only allowed in Cancelled state",
  143: "No contribution to refund",
  150: "Legal hold clear request missing",
  151: "Legal hold clear delay not elapsed",
  152: "Legal hold clear delay overflow",
  160: "Legal hold blocks beneficiary rotation",
  161: "Beneficiary rotation not permitted in current state",
  162: "New SME must differ from current beneficiary",
  163: "No pending admin",
  164: "Funding deadline has passed",
};

/** Category groupings for error codes to help SDK consumers branch on categories. */
export const ESCROW_ERROR_CATEGORIES: Record<string, { range: [number, number]; label: string }> = {
  init: { range: [1, 13], label: "Init / pricing configuration" },
  uninitialized: { range: [20, 22], label: "Uninitialized escrow metadata" },
  dustSweep: { range: [30, 42], label: "Dust sweep / token safety" },
  attestation: { range: [50, 51], label: "Attestation failure" },
  collateral: { range: [60, 62], label: "Collateral metadata failure" },
  adminValidation: { range: [70, 83], label: "Administrative validation" },
  migration: { range: [90, 92], label: "Schema migration failure" },
  funding: { range: [100, 111], label: "Funding failure" },
  settlement: { range: [120, 129], label: "Settlement / payout failure" },
  cancelRefund: { range: [140, 143], label: "Cancel / refund failure" },
  legalHoldClear: { range: [150, 152], label: "Legal hold clear workflow failure" },
  beneficiary: { range: [160, 164], label: "Beneficiary / admin / deadline failure" },
};

/**
 * Classify an error code into a category.
 * Returns the category key or "unknown" if the code falls outside defined ranges.
 */
export function classifyError(code: number): string {
  for (const [key, { range }] of Object.entries(ESCROW_ERROR_CATEGORIES)) {
    if (code >= range[0] && code <= range[1]) return key;
  }
  return "unknown";
}

// ---------------------------------------------------------------------------
// Init parameter helpers
// ---------------------------------------------------------------------------

/**
 * Parameters for the `init` entrypoint.
 * All Soroban numeric types are represented as strings to preserve precision.
 */
export interface InitParams {
  admin: string;
  invoice_id: string;
  sme_address: string;
  amount: string; // i128 — token base units (e.g. "10000_0000000")
  yield_bps: string; // i64 — 800 = 8%
  maturity: string; // u64 — 0 = no maturity gate
  funding_token: string;
  registry: string | null;
  treasury: string;
  yield_tiers: YieldTier[] | null;
  min_contribution: string | null;
  max_unique_investors: number | null;
  max_per_investor: string | null;
  legal_hold_clear_delay: string | null;
  funding_deadline: string | null;
  yield_slippage_threshold: string | null;
}

/**
 * Utility to compute a Stellar base-unit amount from a human-readable decimal.
 * Example: toBaseUnits("10000", 7) → "100000000000" (10000_0000000)
 */
export function toBaseUnits(decimalAmount: string, decimals: number = 7): string {
  const parts = decimalAmount.split(".");
  const whole = parts[0];
  const frac = (parts[1] || "").padEnd(decimals, "0").slice(0, decimals);
  // Remove leading zeros from whole part
  const trimmed = (whole + frac).replace(/^0+/, "") || "0";
  return trimmed;
}

/**
 * Utility to format a base-unit amount back to a human-readable decimal.
 * Example: fromBaseUnits("100000000000", 7) → "10000.0"
 */
export function fromBaseUnits(baseUnits: string, decimals: number = 7): string {
  const padded = baseUnits.padStart(decimals + 1, "0");
  const intPart = padded.slice(0, padded.length - decimals) || "0";
  const fracPart = padded.slice(padded.length - decimals).replace(/0+$/, "");
  return fracPart ? `${intPart}.${fracPart}` : intPart;
}

// ---------------------------------------------------------------------------
// Soroban XDR / result parsing helpers
// ---------------------------------------------------------------------------

/** Represents a parsed Soroban contract invocation result. */
export interface SorobanResult<T = unknown> {
  ok: boolean;
  value?: T;
  error?: {
    code: EscrowErrorCode;
    message: string;
    diagnostic?: ErrorDiagnostic;
  };
}

/**
 * Parse a raw Soroban host function result into a typed SorobanResult.
 * This is a thin wrapper — actual XDR parsing is delegated to stellar-sdk.
 */
export function parseResult<T>(
  rawResult: unknown,
  parser?: (raw: unknown) => T,
): SorobanResult<T> {
  try {
    // In production, this would decode the Stellar XDR result envelope.
    // For now we provide the interface contract.
    if (parser) {
      return { ok: true, value: parser(rawResult) };
    }
    return { ok: true, value: rawResult as T };
  } catch (e) {
    return {
      ok: false,
      error: {
        code: EscrowErrorCode.EscrowNotInitialized,
        message: String(e),
      },
    };
  }
}
