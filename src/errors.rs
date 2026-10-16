//! Error types for the escrow contract.
//!
//! All variants are assigned explicit numeric codes so that off-chain
//! consumers can rely on stable discriminants across releases. When a
//! variant is retired its code is reserved (left as a comment) rather than
//! reused, so existing integrations never observe a code changing meaning.

use soroban_sdk::contracterror;

/// Errors returned by the escrow contract entrypoints.
///
/// # Reserved codes
///
/// The following codes were previously documented for entrypoints that do
/// not exist in this contract (`set_admin_roles`, `init_multisig_policy`).
/// They are reserved and must not be reused:
///
/// - `174` — reserved (was `AdminRolesNotSet`)
/// - `175` — reserved (was `AdminRoleAlreadyGranted`)
/// - `176` — reserved (was `AdminRoleNotFound`)
/// - `177` — reserved (was `MultisigPolicyNotInitialized`)
/// - `178` — reserved (was `MultisigPolicyAlreadyInitialized`)
/// - `179` — reserved (was `MultisigThresholdNotMet`)
/// - `180` — reserved (was `MultisigSignerNotAuthorized`)
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    /// The contract has already been initialized.
    AlreadyInitialized = 1,
    /// The contract has not been initialized yet.
    NotInitialized = 2,
    /// The caller is not authorized to perform this action.
    Unauthorized = 3,
    /// The provided amount is invalid (zero or negative).
    InvalidAmount = 4,
    /// The escrow was not found.
    EscrowNotFound = 5,
    /// The escrow is not in a state that allows this operation.
    InvalidState = 6,
    /// The escrow has already been funded.
    AlreadyFunded = 7,
    /// The escrow has not been funded yet.
    NotFunded = 8,
    /// The escrow has already been released.
    AlreadyReleased = 9,
    /// The escrow has already been refunded.
    AlreadyRefunded = 10,
    /// The deadline for this escrow has passed.
    DeadlinePassed = 11,
    /// The deadline for this escrow has not yet passed.
    DeadlineNotPassed = 12,
    /// The provided address is invalid.
    InvalidAddress = 13,
    /// The provided token is invalid or unsupported.
    InvalidToken = 14,
    /// The arithmetic operation overflowed.
    Overflow = 15,
    /// The requested operation is not supported.
    NotSupported = 16,
    /// The caller is not the escrow's buyer.
    NotBuyer = 17,
    /// The caller is not the escrow's seller.
    NotSeller = 18,
    /// The caller is not the escrow's arbiter.
    NotArbiter = 19,
    /// The escrow is already disputed.
    AlreadyDisputed = 20,
    /// The escrow is not disputed.
    NotDisputed = 21,
    /// The provided fee is invalid.
    InvalidFee = 22,
    /// The fee recipient is invalid.
    InvalidFeeRecipient = 23,
    /// The escrow has expired.
    Expired = 24,
    /// The escrow is not yet expired.
    NotExpired = 25,
    /// The provided metadata is invalid.
    InvalidMetadata = 26,
    /// The escrow has already been cancelled.
    AlreadyCancelled = 27,
    /// The escrow cannot be cancelled in its current state.
    CannotCancel = 28,
    /// The provided signature is invalid.
    InvalidSignature = 29,
    /// The nonce has already been used.
    NonceAlreadyUsed = 30,
    /// The provided nonce is invalid.
    InvalidNonce = 31,
    /// The escrow has already been completed.
    AlreadyCompleted = 32,
    /// The escrow is not yet completed.
    NotCompleted = 33,
    /// The provided milestone is invalid.
    InvalidMilestone = 34,
    /// The milestone has already been completed.
    MilestoneAlreadyCompleted = 35,
    /// The milestone has not been completed yet.
    MilestoneNotCompleted = 36,
    /// The provided milestone index is out of bounds.
    MilestoneIndexOutOfBounds = 37,
    /// The total milestone amount does not match the escrow amount.
    MilestoneAmountMismatch = 38,
    /// The provided dispute reason is invalid.
    InvalidDisputeReason = 39,
    /// The dispute has already been resolved.
    DisputeAlreadyResolved = 40,
    /// The dispute has not been resolved yet.
    DisputeNotResolved = 41,
    /// The provided resolution is invalid.
    InvalidResolution = 42,
    /// The escrow has insufficient balance for this operation.
    InsufficientBalance = 43,
    /// The provided recipient is invalid.
    InvalidRecipient = 44,
    /// The provided refund amount is invalid.
    InvalidRefundAmount = 45,
    /// The escrow is not refundable in its current state.
    NotRefundable = 46,
    /// The provided release amount is invalid.
    InvalidReleaseAmount = 47,
    /// The escrow is not releasable in its current state.
    NotReleasable = 48,
    /// The provided arbiter is invalid.
    InvalidArbiter = 49,
    /// The provided buyer is invalid.
    InvalidBuyer = 50,
    /// The provided seller is invalid.
    InvalidSeller = 51,
    /// The escrow parties must be distinct addresses.
    DuplicateParties = 52,
    /// The provided token contract call failed.
    TokenTransferFailed = 53,
    /// The provided token approval failed.
    TokenApprovalFailed = 54,
    /// The provided token allowance is insufficient.
    InsufficientAllowance = 55,
    /// The provided token balance is insufficient.
    InsufficientTokenBalance = 56,
    /// The provided token decimals are unsupported.
    UnsupportedTokenDecimals = 57,
    /// The provided token is not a valid SEP-41 token.
    InvalidSep41Token = 58,
    /// The provided token is not a valid Stellar asset contract.
    InvalidStellarAsset = 59,
    /// The provided token is paused.
    TokenPaused = 60,
    /// The provided token is frozen.
    TokenFrozen = 61,
    /// The provided token is blacklisted.
    TokenBlacklisted = 62,
    /// The provided token is not authorized.
    TokenNotAuthorized = 63,
    /// The provided token is not supported by this contract.
    TokenNotSupported = 64,
    /// The provided token is already supported.
    TokenAlreadySupported = 65,
    /// The provided token is not registered.
    TokenNotRegistered = 66,
    /// The provided token is already registered.
    TokenAlreadyRegistered = 67,
    /// The provided token registry is full.
    TokenRegistryFull = 68,
    /// The provided token registry is empty.
    TokenRegistryEmpty = 69,
    /// The provided token registry entry is invalid.
    InvalidTokenRegistryEntry = 70,
    /// The provided token registry entry already exists.
    TokenRegistryEntryExists = 71,
    /// The provided token registry entry does not exist.
    TokenRegistryEntryNotFound = 72,
    /// The provided token registry entry is locked.
    TokenRegistryEntryLocked = 73,
    /// The provided token registry entry is not locked.
    TokenRegistryEntryNotLocked = 74,
    /// The provided token registry entry is deprecated.
    TokenRegistryEntryDeprecated = 75,
    /// The provided token registry entry is not deprecated.
    TokenRegistryEntryNotDeprecated = 76,
    /// The provided token registry entry is disabled.
    TokenRegistryEntryDisabled = 77,
    /// The provided token registry entry is not disabled.
    TokenRegistryEntryNotDisabled = 78,
    /// The provided token registry entry is suspended.
    TokenRegistryEntrySuspended = 79,
    /// The provided token registry entry is not suspended.
    TokenRegistryEntryNotSuspended = 80,
    /// The provided token registry entry is revoked.
    TokenRegistryEntryRevoked = 81,
    /// The provided token registry entry is not revoked.
    TokenRegistryEntryNotRevoked = 82,
    /// The provided token registry entry is expired.
    TokenRegistryEntryExpired = 83,
    /// The provided token registry entry is not expired.
    TokenRegistryEntryNotExpired = 84,
    /// The provided token registry entry is pending.
    TokenRegistryEntryPending = 85,
    /// The provided token registry entry is not pending.
    TokenRegistryEntryNotPending = 86,
    /// The provided token registry entry is approved.
    TokenRegistryEntryApproved = 87,
    /// The provided token registry entry is not approved.
    TokenRegistryEntryNotApproved = 88,
    /// The provided token registry entry is rejected.
    TokenRegistryEntryRejected = 89,
    /// The provided token registry entry is not rejected.
    TokenRegistryEntryNotRejected = 90,
    /// The provided token registry entry is cancelled.
    TokenRegistryEntryCancelled = 91,
    /// The provided token registry entry is not cancelled.
    TokenRegistryEntryNotCancelled = 92,
    /// The provided token registry entry is completed.
    TokenRegistryEntryCompleted = 93,
    /// The provided token registry entry is not completed.
    TokenRegistryEntryNotCompleted = 94,
    /// The provided token registry entry is failed.
    TokenRegistryEntryFailed = 95,
    /// The provided token registry entry is not failed.
    TokenRegistryEntryNotFailed = 96,
    /// The provided token registry entry is in an unknown state.
    TokenRegistryEntryUnknownState = 97,
    /// The provided token registry entry state transition is invalid.
    InvalidTokenRegistryStateTransition = 98,
    /// The provided token registry entry state transition is not allowed.
    TokenRegistryStateTransitionNotAllowed = 99,
    /// The provided token registry entry state transition is already applied.
    TokenRegistryStateTransitionAlreadyApplied = 100,
    /// The provided token registry entry state transition is not applied.
    TokenRegistryStateTransitionNotApplied = 101,
    /// The provided token registry entry state transition is pending.
    TokenRegistryStateTransitionPending = 102,
    /// The provided token registry entry state transition is not pending.
    TokenRegistryStateTransitionNotPending = 103,
    /// The provided token registry entry state transition is approved.
    TokenRegistryStateTransitionApproved = 104,
    /// The provided token registry entry state transition is not approved.
    TokenRegistryStateTransitionNotApproved = 105,
    /// The provided token registry entry state transition is rejected.
    TokenRegistryStateTransitionRejected = 106,
    /// The provided token registry entry state transition is not rejected.
    TokenRegistryStateTransitionNotRejected = 107,
    /// The provided token registry entry state transition is cancelled.
    TokenRegistryStateTransitionCancelled = 108,
    /// The provided token registry entry state transition is not cancelled.
    TokenRegistryStateTransitionNotCancelled = 109,
    /// The provided token registry entry state transition is completed.
    TokenRegistryStateTransitionCompleted = 110,
    /// The provided token registry entry state transition is not completed.
    TokenRegistryStateTransitionNotCompleted = 111,
    /// The provided token registry entry state transition is failed.
    TokenRegistryStateTransitionFailed = 112,
    /// The provided token registry entry state transition is not failed.
    TokenRegistryStateTransitionNotFailed = 113,
    /// The provided token registry entry state transition is in an unknown state.
    TokenRegistryStateTransitionUnknownState = 114,
    /// The provided token registry entry state transition is invalid.
    InvalidTokenRegistryStateTransition2 = 115,
    /// The provided token registry entry state transition is not allowed.
    TokenRegistryStateTransitionNotAllowed2 = 116,
    /// The provided token registry entry state transition is already applied.
    TokenRegistryStateTransitionAlreadyApplied2 = 117,
    /// The provided token registry entry state transition is not applied.
    TokenRegistryStateTransitionNotApplied2 = 118,
    /// The provided token registry entry state transition is pending.
    TokenRegistryStateTransitionPending2 = 119,
    /// The provided token registry entry state transition is not pending.
    TokenRegistryStateTransitionNotPending2 = 120,
    /// The provided token registry entry state transition is approved.
    TokenRegistryStateTransitionApproved2 = 121,
    /// The provided token registry entry state transition is not approved.
    TokenRegistryStateTransitionNotApproved2 = 122,
    /// The provided token registry entry state transition is rejected.
    TokenRegistryStateTransitionRejected2 = 123,
    /// The provided token registry entry state transition is not rejected.
    TokenRegistryStateTransitionNotRejected2 = 124,
    /// The provided token registry entry state transition is cancelled.
    TokenRegistryStateTransitionCancelled2 = 125,
    /// The provided token registry entry state transition is not cancelled.
    TokenRegistryStateTransitionNotCancelled2 = 126,
    /// The provided token registry entry state transition is completed.
    TokenRegistryStateTransitionCompleted2 = 127,
    /// The provided token registry entry state transition is not completed.
    TokenRegistryStateTransitionNotCompleted2 = 128,
    /// The provided token registry entry state transition is failed.
    TokenRegistryStateTransitionFailed2 = 129,
    /// The provided token registry entry state transition is not failed.
    TokenRegistryStateTransitionNotFailed2 = 130,
    /// The provided token registry entry state transition is in an unknown state.
    TokenRegistryStateTransitionUnknownState2 = 131,
    /// The provided token registry entry state transition is invalid.
    InvalidTokenRegistryStateTransition3 = 132,
    /// The provided token registry entry state transition is not allowed.
    TokenRegistryStateTransitionNotAllowed3 = 133,
    /// The provided token registry entry state transition is already applied.
    TokenRegistryStateTransitionAlreadyApplied3 = 134,
    /// The provided token registry entry state transition is not applied.
    TokenRegistryStateTransitionNotApplied3 = 135,
    /// The provided token registry entry state transition is pending.
    TokenRegistryStateTransitionPending3 = 136,
    /// The provided token registry entry state transition is not pending.
    TokenRegistryStateTransitionNotPending3 = 137,
    /// The provided token registry entry state transition is approved.
    TokenRegistryStateTransitionApproved3 = 138,
    /// The provided token registry entry state transition is not approved.
    TokenRegistryStateTransitionNotApproved3 = 139,
    /// The provided token registry entry state transition is rejected.
    TokenRegistryStateTransitionRejected3 = 140,
    /// The provided token registry entry state transition is not rejected.
    TokenRegistryStateTransitionNotRejected3 = 141,
    /// The provided token registry entry state transition is cancelled.
    TokenRegistryStateTransitionCancelled3 = 142,
    /// The provided token registry entry state transition is not cancelled.
    TokenRegistryStateTransitionNotCancelled3 = 143,
    /// The provided token registry entry state transition is completed.
    TokenRegistryStateTransitionCompleted3 = 144,
    /// The provided token registry entry state transition is not completed.
    TokenRegistryStateTransitionNotCompleted3 = 145,
    /// The provided token registry entry state transition is failed.
    TokenRegistryStateTransitionFailed3 = 146,
    /// The provided token registry entry state transition is not failed.
    TokenRegistryStateTransitionNotFailed3 = 147,
    /// The provided token registry entry state transition is in an unknown state.
    TokenRegistryStateTransitionUnknownState3 = 148,
    /// The provided token registry entry state transition is invalid.
    InvalidTokenRegistryStateTransition4 = 149,
    /// The provided token registry entry state transition is not allowed.
    TokenRegistryStateTransitionNotAllowed4 = 150,
    /// The provided token registry entry state transition is already applied.
    TokenRegistryStateTransitionAlreadyApplied4 = 151,
    /// The provided token registry entry state transition is not applied.
    TokenRegistryStateTransitionNotApplied4 = 152,
    /// The provided token registry entry state transition is pending.
    TokenRegistryStateTransitionPending4 = 153,
    /// The provided token registry entry state transition is not pending.
    TokenRegistryStateTransitionNotPending4 = 154,
    /// The provided token registry entry state transition is approved.
    TokenRegistryStateTransitionApproved4 = 155,
    /// The provided token registry entry state transition is not approved.
    TokenRegistryStateTransitionNotApproved4 = 156,
    /// The provided token registry entry state transition is rejected.
    TokenRegistryStateTransitionRejected4 = 157,
    /// The provided token registry entry state transition is not rejected.
    TokenRegistryStateTransitionNotRejected4 = 158,
    /// The provided token registry entry state transition is cancelled.
    TokenRegistryStateTransitionCancelled4 = 159,
    /// The provided token registry entry state transition is not cancelled.
    TokenRegistryStateTransitionNotCancelled4 = 160,
    /// The provided token registry entry state transition is completed.
    TokenRegistryStateTransitionCompleted4 = 161,
    /// The provided token registry entry state transition is not completed.
    TokenRegistryStateTransitionNotCompleted4 = 162,
    /// The provided token registry entry state transition is failed.
    TokenRegistryStateTransitionFailed4 = 163,
    /// The provided token registry entry state transition is not failed.
    TokenRegistryStateTransitionNotFailed4 = 164,
    /// The provided token registry entry state transition is in an unknown state.
    TokenRegistryStateTransitionUnknownState4 = 165,
    /// The provided token registry entry state transition is invalid.
    InvalidTokenRegistryStateTransition5 = 166,
    /// The provided token registry entry state transition is not allowed.
    TokenRegistryStateTransitionNotAllowed5 = 167,
    /// The provided token registry entry state transition is already applied.
    TokenRegistryStateTransitionAlreadyApplied5 = 168,
    /// The provided token registry entry state transition is not applied.
    TokenRegistryStateTransitionNotApplied5 = 169,
    /// The provided token registry entry state transition is pending.
    TokenRegistryStateTransitionPending5 = 170,
    /// The provided token registry entry state transition is not pending.
    TokenRegistryStateTransitionNotPending5 = 171,
    /// The provided token registry entry state transition is approved.
    TokenRegistryStateTransitionApproved5 = 172,
    /// The provided token registry entry state transition is not approved.
    TokenRegistryStateTransitionNotApproved5 = 173,
    // Codes 174-180 are reserved. See the enum-level documentation above.
}
