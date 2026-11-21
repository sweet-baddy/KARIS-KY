// karis-ky Escrow SDK — Operational Limits and Constants
//
// Provides typed access to contract-level operational limits for validation
// and batch size calculations. These constants are also available at runtime
// via the `getLimits()` method for dynamic limit retrieval.
//
// Schema version: 8
// Interface version: 1

/**
 * Batch size limit for `fundBatch()` and `fund_batch()` operations.
 * Attempting to batch more than this many entries in a single call will fail.
 *
 * Use this to split large funding batches into multiple transactions.
 *
 * @example
 * const entries = [...]; // 100 entries
 * const chunks = [];
 * for (let i = 0; i < entries.length; i += MAX_FUND_BATCH) {
 *   chunks.push(entries.slice(i, i + MAX_FUND_BATCH));
 * }
 * for (const chunk of chunks) {
 *   await client.batchFund(chunk);
 * }
 */
export const MAX_FUND_BATCH = 50;

/**
 * Batch size limit for `batchClaimInvestorPayouts()` operations.
 * Attempting to claim payout for more than this many investors in a single call will fail.
 */
export const MAX_BATCH_CLAIM = 50;

/**
 * Maximum number of entries allowed in a single `appendAttestationDigest()` accumulation.
 * The attestation log can hold up to 32 distinct 32-byte digests.
 *
 * Use this to rate-limit attestation appends or detect if the log has reached capacity.
 */
export const MAX_ATTESTATION_APPEND_ENTRIES = 32;

/**
 * Maximum number of entries allowed in a single batch allowlist mutation.
 * Both `setInvestorsAllowlisted()` add and remove operations are subject to this limit.
 */
export const MAX_INVESTOR_ALLOWLIST_BATCH = 32;

/**
 * Maximum number of funding history entries per investor.
 * The history includes deposits, funding close snapshots, and settlement checkpoints.
 * Once this limit is reached for an investor, older entries are pruned.
 */
export const MAX_INVESTOR_HISTORY_ENTRIES = 128;

/**
 * Maximum number of entries returned by a single `getInvestorHistoryPage()` call.
 * Use pagination for investors with history larger than this.
 */
export const MAX_HISTORY_PAGE_SIZE = 50;

/**
 * Maximum amount (in token base units) that can be swept as terminal dust in a single call.
 * This prevents accidental large sweeps and is a safety guard on treasury withdrawals.
 */
export const MAX_DUST_SWEEP_AMOUNT = "100000000";

/**
 * Maximum number of reinvestment audit log entries tracked per escrow.
 * Older entries are pruned once this limit is reached.
 */
export const MAX_REINVESTMENT_AUDIT_ENTRIES = 1024;

/**
 * Maximum number of admin role assignments for multi-signature or delegated governance.
 */
export const MAX_ADMIN_ROLES = 16;

/**
 * Maximum number of signers allowed in a single multisig policy.
 */
export const MAX_MULTISIG_SIGNERS = 10;

/**
 * Maximum number of distinct operations allowed in a multisig policy.
 */
export const MAX_MULTISIG_OPERATIONS = 50;

// ---------------------------------------------------------------------------
// Invoice ID validation constants
// ---------------------------------------------------------------------------

/**
 * Maximum length (in bytes and characters) for an invoice ID.
 * Must be 1..=32 ASCII characters; allowed charset: [A-Za-z0-9_]
 */
export const MAX_INVOICE_ID_STRING_LEN = 32;

/**
 * Minimum length (in bytes and characters) for an invoice ID.
 */
export const MIN_INVOICE_ID_STRING_LEN = 1;

/**
 * Regex pattern for valid invoice IDs: ASCII alphanumeric plus underscore only.
 */
export const INVOICE_ID_PATTERN = /^[A-Za-z0-9_]+$/;

// ---------------------------------------------------------------------------
// Schema and interface versioning
// ---------------------------------------------------------------------------

/**
 * Schema version of this SDK release.
 * Must match the deployed contract's schema version for full compatibility.
 */
export const SCHEMA_VERSION = 8;

/**
 * Contract interface version.
 * When this changes, all client implementations must be regenerated.
 */
export const CONTRACT_INTERFACE_VERSION = 1;
