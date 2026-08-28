// karis-ky Escrow SDK — Public API
//
// Re-exports all types, constants, and the client class.

export {
  EscrowClient,
  type EscrowClientConfig,
  type SorobanRpcClient,
  type SorobanEventFilter,
  type SorobanEventQuery,
} from "./client";

export {
  EscrowStatus,
  ESCROW_STATUS_LABELS,
  EscrowErrorCode,
  ESCROW_ERROR_LABELS,
  ESCROW_ERROR_CATEGORIES,
  classifyError,
  SCHEMA_VERSION,
  CONTRACT_INTERFACE_VERSION,
  MAX_INVOICE_ID_STRING_LEN,
  MAX_ATTESTATION_APPEND_ENTRIES,
  MAX_INVESTOR_ALLOWLIST_BATCH,
  MAX_FUND_BATCH,
  MAX_DUST_SWEEP_AMOUNT,
  toBaseUnits,
  fromBaseUnits,
  parseResult,
  type InvoiceEscrow,
  type YieldTier,
  type FundingCloseSnapshot,
  type SmeCollateralCommitment,
  type EscrowSummary,
  type ErrorDiagnostic,
  type EscrowTemplate,
  type InitParams,
  type SorobanResult,
  type EscrowEvent,
  type SorobanEventPage,
  type EscrowEventSubscriptionOptions,
} from "./types";
