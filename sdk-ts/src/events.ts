// karis-ky Escrow SDK — Typed Event Parser
//
// Maps raw Soroban contract events (EscrowEvent) to typed structs mirroring
// docs/EVENT_SCHEMA.md. Routing is done on `topic[1]`, the karis-ky short
// event symbol (e.g. "funded", "escrow_sd").
//
// Usage:
//   import { parseEvent } from "@karis-ky/escrow-sdk";
//   const parsed = parseEvent(rawEvent);
//   if (parsed.type === "EscrowFunded") { ... parsed.data.amount ... }

import type { EscrowEvent, InvoiceEscrow } from "./types";

// ---------------------------------------------------------------------------
// Typed event structs (one per event in docs/EVENT_SCHEMA.md)
// ---------------------------------------------------------------------------

export interface EscrowInitializedEvent {
  type: "EscrowInitialized";
  /** Short routing symbol: `escrow_ii` */
  name: "escrow_ii";
  data: {
    escrow: InvoiceEscrow;
    funding_token: string;
    treasury: string;
    registry: string | null;
    has_maturity_lock: boolean;
  };
}

export interface MaxUniqueInvestorsCapLoweredEvent {
  type: "MaxUniqueInvestorsCapLowered";
  /** Short routing symbol: `inv_cap` */
  name: "inv_cap";
  invoice_id: string;
  data: {
    old_cap: number;
    new_cap: number;
  };
}

export interface EscrowFundedEvent {
  type: "EscrowFunded";
  /** Short routing symbol: `funded` */
  name: "funded";
  invoice_id: string;
  investor: string;
  data: {
    amount: string;
    funded_amount: string;
    status: number;
    investor_effective_yield_bps: string;
  };
}

export interface EscrowSettledEvent {
  type: "EscrowSettled";
  /** Short routing symbol: `escrow_sd` */
  name: "escrow_sd";
  invoice_id: string;
  data: {
    funded_amount: string;
    yield_bps: string;
    maturity: string;
    settled_at_ledger_timestamp: string;
  };
}

export interface MaturityUpdatedEvent {
  type: "MaturityUpdated";
  /** Short routing symbol: `maturity` */
  name: "maturity";
  invoice_id: string;
  data: {
    old_maturity: string;
    new_maturity: string;
  };
}

export interface AdminTransferredEvent {
  type: "AdminTransferred";
  /** Short routing symbol: `admin` */
  name: "admin";
  invoice_id: string;
  data: {
    new_admin: string;
  };
}

export interface AdminProposedEvent {
  type: "AdminProposed";
  /** Short routing symbol: `adm_prop` */
  name: "adm_prop";
  invoice_id: string;
  data: {
    current_admin: string;
    pending_admin: string;
  };
}

export interface BeneficiaryRotatedEvent {
  type: "BeneficiaryRotated";
  /** Short routing symbol: `ben_rot` */
  name: "ben_rot";
  invoice_id: string;
  data: {
    prior_sme: string;
    new_sme: string;
  };
}

export interface FundingTargetUpdatedEvent {
  type: "FundingTargetUpdated";
  /** Short routing symbol: `fund_tgt` */
  name: "fund_tgt";
  invoice_id: string;
  data: {
    old_target: string;
    new_target: string;
  };
}

export interface LegalHoldChangedEvent {
  type: "LegalHoldChanged";
  /** Short routing symbol: `legalhld` */
  name: "legalhld";
  invoice_id: string;
  data: {
    /** `1` = hold active, `0` = hold cleared */
    active: number;
  };
}

export interface DisputePausedEvent {
  type: "DisputePaused";
  /** Short routing symbol: `disppause` */
  name: "disppause";
  invoice_id: string;
  data: {
    ticket_id: string;
    /** Always `1` (paused) for this event variant */
    action: number;
    paused_at: string;
    expires_at: string;
  };
}

export interface DisputeResumedEvent {
  type: "DisputeResumed";
  /** Short routing symbol: `disp_res` */
  name: "disp_res";
  invoice_id: string;
  data: {
    admin: string;
    /** `"Manual"` or `"AutoExpiry"` */
    resumed_by: string;
    ledger_timestamp: string;
  };
}

export interface LegalHoldProposedEvent {
  type: "LegalHoldProposed";
  /** Short routing symbol: `lh_prop` */
  name: "lh_prop";
  data: {
    admin: string;
    expires_at: string;
  };
}

export interface CollateralRecordedEvent {
  type: "CollateralRecorded";
  /** Short routing symbol: `coll_rec` */
  name: "coll_rec";
  data: {
    invoice_id: string;
    amount: string;
    prior_amount: string;
  };
}

export interface SmeWithdrewEvent {
  type: "SmeWithdrew";
  /** Short routing symbol: `sme_wd` */
  name: "sme_wd";
  invoice_id: string;
  data: {
    amount: string;
    recipient: string;
  };
}

export interface InvestorPayoutClaimedEvent {
  type: "InvestorPayoutClaimed";
  /** Short routing symbol: `inv_claim` */
  name: "inv_claim";
  invoice_id: string;
  investor: string;
  data: Record<string, never>;
}

export interface FundingCancelledEvent {
  type: "FundingCancelled";
  /** Short routing symbol: `fund_can` */
  name: "fund_can";
  invoice_id: string;
  data: {
    funded_amount: string;
  };
}

export interface InvestorRefundedEvent {
  type: "InvestorRefunded";
  /** Short routing symbol: `refunded` */
  name: "refunded";
  invoice_id: string;
  investor: string;
  data: {
    amount: string;
  };
}

export interface TreasuryDustSweptEvent {
  type: "TreasuryDustSwept";
  /** Short routing symbol: `dust_sw` */
  name: "dust_sw";
  data: {
    invoice_id: string;
    token: string;
    amount: string;
  };
}

export interface PrimaryAttestationBoundEvent {
  type: "PrimaryAttestationBound";
  /** Short routing symbol: `att_bind` */
  name: "att_bind";
  data: {
    invoice_id: string;
    /** 64 hex characters */
    digest: string;
  };
}

export interface AttestationBoundEvent {
  type: "AttestationBound";
  /** Short routing symbol: `att_bound` */
  name: "att_bound";
  data: {
    /** 64 hex characters */
    hash: string;
    /** Decimal string */
    ledger_timestamp: string;
  };
}

export interface AttestationDigestAppendedEvent {
  type: "AttestationDigestAppended";
  /** Short routing symbol: `att_app` */
  name: "att_app";
  data: {
    invoice_id: string;
    index: number;
    /** 64 hex characters */
    digest: string;
  };
}

export interface AllowlistEnabledChangedEvent {
  type: "AllowlistEnabledChanged";
  /** Short routing symbol: `al_ena` */
  name: "al_ena";
  data: {
    invoice_id: string;
    /** `1` = enabled, `0` = disabled */
    active: number;
  };
}

export interface InvestorAllowlistChangedEvent {
  type: "InvestorAllowlistChanged";
  /** Short routing symbol: `al_set` */
  name: "al_set";
  data: {
    invoice_id: string;
    investor: string;
    /** `1` = allowed, `0` = blocked */
    allowed: number;
  };
}

/** Returned when the event's `name` symbol is not in the known catalog. */
export interface UnknownEvent {
  type: "Unknown";
  name: string;
  raw: EscrowEvent;
}

// ---------------------------------------------------------------------------
// Discriminated union of all typed events
// ---------------------------------------------------------------------------

export type ParsedEscrowEvent =
  | EscrowInitializedEvent
  | MaxUniqueInvestorsCapLoweredEvent
  | EscrowFundedEvent
  | EscrowSettledEvent
  | MaturityUpdatedEvent
  | AdminTransferredEvent
  | AdminProposedEvent
  | BeneficiaryRotatedEvent
  | FundingTargetUpdatedEvent
  | LegalHoldChangedEvent
  | DisputePausedEvent
  | DisputeResumedEvent
  | LegalHoldProposedEvent
  | CollateralRecordedEvent
  | SmeWithdrewEvent
  | InvestorPayoutClaimedEvent
  | FundingCancelledEvent
  | InvestorRefundedEvent
  | TreasuryDustSweptEvent
  | PrimaryAttestationBoundEvent
  | AttestationBoundEvent
  | AttestationDigestAppendedEvent
  | AllowlistEnabledChangedEvent
  | InvestorAllowlistChangedEvent
  | UnknownEvent;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Extract the karis-ky routing symbol from topic[1].
 * Returns an empty string when the topics array is too short or the value is
 * not a string.
 */
function extractName(event: EscrowEvent): string {
  const topics = event.topics;
  if (!Array.isArray(topics) || topics.length < 2) return "";
  const name = topics[1];
  return typeof name === "string" ? name : "";
}

/**
 * Retrieve a field from the event data payload.
 * Accepts both a plain object and the `{ _map: [...] }` envelope that some
 * Soroban RPC adapters produce after XDR decoding.
 */
function field(value: unknown, key: string): unknown {
  if (value === null || typeof value !== "object") return undefined;
  const obj = value as Record<string, unknown>;
  if (typeof obj[key] !== "undefined") return obj[key];

  // Support the { _map: [[key, value], ...] } envelope from some adapters.
  if (Array.isArray(obj["_map"])) {
    for (const entry of obj["_map"] as Array<[unknown, unknown]>) {
      if (Array.isArray(entry) && entry[0] === key) return entry[1];
    }
  }
  return undefined;
}

function str(v: unknown): string {
  if (typeof v === "string") return v;
  if (typeof v === "number" || typeof v === "bigint") return String(v);
  return "";
}

function num(v: unknown): number {
  if (typeof v === "number") return v;
  if (typeof v === "string") return Number(v);
  return 0;
}

// ---------------------------------------------------------------------------
// parseEvent
// ---------------------------------------------------------------------------

/**
 * Parse a raw {@link EscrowEvent} into a typed {@link ParsedEscrowEvent}.
 *
 * Routing is done on `topics[1]` (the karis-ky short event symbol). Fields are
 * extracted from `event.value` using both plain-object and `_map` envelope
 * formats so the function works with any Soroban RPC adapter.
 *
 * Returns an {@link UnknownEvent} for any symbol not present in the catalog
 * rather than throwing, so callers can safely ignore unknown variants.
 *
 * @example
 * ```ts
 * import { parseEvent } from "@karis-ky/escrow-sdk";
 *
 * const parsed = parseEvent(rawEvent);
 * if (parsed.type === "EscrowFunded") {
 *   console.log(parsed.invoice_id, parsed.data.amount);
 * }
 * ```
 */
export function parseEvent(event: EscrowEvent): ParsedEscrowEvent {
  const name = extractName(event);
  const v = event.value;

  switch (name) {
    case "escrow_ii":
      return {
        type: "EscrowInitialized",
        name: "escrow_ii",
        data: {
          escrow: field(v, "escrow") as InvoiceEscrow,
          funding_token: str(field(v, "funding_token")),
          treasury: str(field(v, "treasury")),
          registry: (field(v, "registry") as string | null) ?? null,
          has_maturity_lock: Boolean(field(v, "has_maturity_lock")),
        },
      };

    case "inv_cap": {
      const topics = event.topics as string[];
      return {
        type: "MaxUniqueInvestorsCapLowered",
        name: "inv_cap",
        invoice_id: str(topics[2]),
        data: {
          old_cap: num(field(v, "old_cap")),
          new_cap: num(field(v, "new_cap")),
        },
      };
    }

    case "funded": {
      const topics = event.topics as string[];
      return {
        type: "EscrowFunded",
        name: "funded",
        invoice_id: str(topics[2]),
        investor: str(topics[3]),
        data: {
          amount: str(field(v, "amount")),
          funded_amount: str(field(v, "funded_amount")),
          status: num(field(v, "status")),
          investor_effective_yield_bps: str(field(v, "investor_effective_yield_bps")),
        },
      };
    }

    case "escrow_sd": {
      const topics = event.topics as string[];
      return {
        type: "EscrowSettled",
        name: "escrow_sd",
        invoice_id: str(topics[2]),
        data: {
          funded_amount: str(field(v, "funded_amount")),
          yield_bps: str(field(v, "yield_bps")),
          maturity: str(field(v, "maturity")),
          settled_at_ledger_timestamp: str(field(v, "settled_at_ledger_timestamp")),
        },
      };
    }

    case "maturity": {
      const topics = event.topics as string[];
      return {
        type: "MaturityUpdated",
        name: "maturity",
        invoice_id: str(topics[2]),
        data: {
          old_maturity: str(field(v, "old_maturity")),
          new_maturity: str(field(v, "new_maturity")),
        },
      };
    }

    case "admin": {
      const topics = event.topics as string[];
      return {
        type: "AdminTransferred",
        name: "admin",
        invoice_id: str(topics[2]),
        data: {
          new_admin: str(field(v, "new_admin")),
        },
      };
    }

    case "adm_prop": {
      const topics = event.topics as string[];
      return {
        type: "AdminProposed",
        name: "adm_prop",
        invoice_id: str(topics[2]),
        data: {
          current_admin: str(field(v, "current_admin")),
          pending_admin: str(field(v, "pending_admin")),
        },
      };
    }

    case "ben_rot": {
      const topics = event.topics as string[];
      return {
        type: "BeneficiaryRotated",
        name: "ben_rot",
        invoice_id: str(topics[2]),
        data: {
          prior_sme: str(field(v, "prior_sme")),
          new_sme: str(field(v, "new_sme")),
        },
      };
    }

    case "fund_tgt": {
      const topics = event.topics as string[];
      return {
        type: "FundingTargetUpdated",
        name: "fund_tgt",
        invoice_id: str(topics[2]),
        data: {
          old_target: str(field(v, "old_target")),
          new_target: str(field(v, "new_target")),
        },
      };
    }

    case "legalhld": {
      const topics = event.topics as string[];
      return {
        type: "LegalHoldChanged",
        name: "legalhld",
        invoice_id: str(topics[2]),
        data: {
          active: num(field(v, "active")),
        },
      };
    }

    case "disppause": {
      const topics = event.topics as string[];
      return {
        type: "DisputePaused",
        name: "disppause",
        invoice_id: str(topics[2]),
        data: {
          ticket_id: str(field(v, "ticket_id")),
          action: num(field(v, "action")),
          paused_at: str(field(v, "paused_at")),
          expires_at: str(field(v, "expires_at")),
        },
      };
    }

    case "disp_res": {
      const topics = event.topics as string[];
      return {
        type: "DisputeResumed",
        name: "disp_res",
        invoice_id: str(topics[2]),
        data: {
          admin: str(field(v, "admin")),
          resumed_by: str(field(v, "resumed_by")),
          ledger_timestamp: str(field(v, "ledger_timestamp")),
        },
      };
    }

    case "lh_prop":
      return {
        type: "LegalHoldProposed",
        name: "lh_prop",
        data: {
          admin: str(field(v, "admin")),
          expires_at: str(field(v, "expires_at")),
        },
      };

    case "coll_rec":
      return {
        type: "CollateralRecorded",
        name: "coll_rec",
        data: {
          invoice_id: str(field(v, "invoice_id")),
          amount: str(field(v, "amount")),
          prior_amount: str(field(v, "prior_amount")),
        },
      };

    case "sme_wd": {
      const topics = event.topics as string[];
      return {
        type: "SmeWithdrew",
        name: "sme_wd",
        invoice_id: str(topics[2]),
        data: {
          amount: str(field(v, "amount")),
          recipient: str(field(v, "recipient")),
        },
      };
    }

    case "inv_claim": {
      const topics = event.topics as string[];
      return {
        type: "InvestorPayoutClaimed",
        name: "inv_claim",
        investor: str(topics[2]),
        invoice_id: str(topics[3]),
        data: {},
      };
    }

    case "fund_can": {
      const topics = event.topics as string[];
      return {
        type: "FundingCancelled",
        name: "fund_can",
        invoice_id: str(topics[2]),
        data: {
          funded_amount: str(field(v, "funded_amount")),
        },
      };
    }

    case "refunded": {
      const topics = event.topics as string[];
      return {
        type: "InvestorRefunded",
        name: "refunded",
        invoice_id: str(topics[3]),
        investor: str(topics[2]),
        data: {
          amount: str(field(v, "amount")),
        },
      };
    }

    case "dust_sw":
      return {
        type: "TreasuryDustSwept",
        name: "dust_sw",
        data: {
          invoice_id: str(field(v, "invoice_id")),
          token: str(field(v, "token")),
          amount: str(field(v, "amount")),
        },
      };

    case "att_bind":
      return {
        type: "PrimaryAttestationBound",
        name: "att_bind",
        data: {
          invoice_id: str(field(v, "invoice_id")),
          digest: str(field(v, "digest")),
        },
      };

    case "att_bound":
      return {
        type: "AttestationBound",
        name: "att_bound",
        data: {
          hash: str(field(v, "hash")),
          ledger_timestamp: str(field(v, "ledger_timestamp")),
        },
      };

    case "att_app":
      return {
        type: "AttestationDigestAppended",
        name: "att_app",
        data: {
          invoice_id: str(field(v, "invoice_id")),
          index: num(field(v, "index")),
          digest: str(field(v, "digest")),
        },
      };

    case "al_ena":
      return {
        type: "AllowlistEnabledChanged",
        name: "al_ena",
        data: {
          invoice_id: str(field(v, "invoice_id")),
          active: num(field(v, "active")),
        },
      };

    case "al_set":
      return {
        type: "InvestorAllowlistChanged",
        name: "al_set",
        data: {
          invoice_id: str(field(v, "invoice_id")),
          investor: str(field(v, "investor")),
          allowed: num(field(v, "allowed")),
        },
      };

    default:
      return {
        type: "Unknown",
        name,
        raw: event,
      };
  }
}
