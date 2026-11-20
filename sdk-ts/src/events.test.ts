/**
 * Unit tests for parseEvent — typed event parser (Issue #78)
 *
 * Each test exercises one event type from docs/EVENT_SCHEMA.md.
 * Coverage: all 24 known event symbols plus the Unknown fallback.
 */

import { parseEvent } from "./events";
import type {
  EscrowInitializedEvent,
  MaxUniqueInvestorsCapLoweredEvent,
  EscrowFundedEvent,
  EscrowSettledEvent,
  MaturityUpdatedEvent,
  AdminTransferredEvent,
  AdminProposedEvent,
  BeneficiaryRotatedEvent,
  FundingTargetUpdatedEvent,
  LegalHoldChangedEvent,
  DisputePausedEvent,
  DisputeResumedEvent,
  LegalHoldProposedEvent,
  CollateralRecordedEvent,
  SmeWithdrewEvent,
  InvestorPayoutClaimedEvent,
  FundingCancelledEvent,
  InvestorRefundedEvent,
  TreasuryDustSweptEvent,
  PrimaryAttestationBoundEvent,
  AttestationBoundEvent,
  AttestationDigestAppendedEvent,
  AllowlistEnabledChangedEvent,
  InvestorAllowlistChangedEvent,
  UnknownEvent,
} from "./events";
import type { EscrowEvent } from "./types";
import { EscrowStatus } from "./types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a minimal EscrowEvent for testing. `name` is placed at topics[1]. */
function mkEvent(
  name: string,
  value: unknown,
  extraTopics: unknown[] = [],
): EscrowEvent {
  return {
    id: `test-${name}`,
    type: "contract",
    contract_id: "CESCROW",
    ledger: 1000,
    ledger_closed_at: "2026-01-01T00:00:00Z",
    paging_token: "1000-0",
    topics: [`${name}_event_topic`, name, ...extraTopics],
    value,
  };
}

// ---------------------------------------------------------------------------
// Tests — one describe block per event type
// ---------------------------------------------------------------------------

describe("parseEvent", () => {
  // =========================================================================
  // EscrowInitialized  (escrow_ii)
  // =========================================================================
  describe("EscrowInitialized (escrow_ii)", () => {
    it("returns the correct variant and maps all data fields", () => {
      const escrow = {
        invoice_id: "INV001",
        admin: "GADMIN",
        sme_address: "GSME",
        amount: "1000000000",
        funding_target: "1000000000",
        funded_amount: "0",
        yield_bps: "800",
        maturity: "1700000000",
        status: EscrowStatus.Open,
        guardian: null,
      };

      const event = mkEvent("escrow_ii", {
        escrow,
        funding_token: "CTOKEN",
        treasury: "CTREASURY",
        registry: null,
        has_maturity_lock: false,
      });

      const parsed = parseEvent(event) as EscrowInitializedEvent;
      expect(parsed.type).toBe("EscrowInitialized");
      expect(parsed.name).toBe("escrow_ii");
      expect(parsed.data.escrow).toEqual(escrow);
      expect(parsed.data.funding_token).toBe("CTOKEN");
      expect(parsed.data.treasury).toBe("CTREASURY");
      expect(parsed.data.registry).toBeNull();
      expect(parsed.data.has_maturity_lock).toBe(false);
    });

    it("sets has_maturity_lock to true when present", () => {
      const event = mkEvent("escrow_ii", {
        escrow: {},
        funding_token: "CTOKEN",
        treasury: "CTREASURY",
        registry: "CREG",
        has_maturity_lock: true,
      });
      const parsed = parseEvent(event) as EscrowInitializedEvent;
      expect(parsed.data.has_maturity_lock).toBe(true);
      expect(parsed.data.registry).toBe("CREG");
    });
  });

  // =========================================================================
  // MaxUniqueInvestorsCapLowered  (inv_cap)
  // =========================================================================
  describe("MaxUniqueInvestorsCapLowered (inv_cap)", () => {
    it("extracts invoice_id from topics[2] and cap values from data", () => {
      const event = mkEvent("inv_cap", { old_cap: 100, new_cap: 50 }, ["INV001"]);
      const parsed = parseEvent(event) as MaxUniqueInvestorsCapLoweredEvent;
      expect(parsed.type).toBe("MaxUniqueInvestorsCapLowered");
      expect(parsed.name).toBe("inv_cap");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.old_cap).toBe(100);
      expect(parsed.data.new_cap).toBe(50);
    });
  });

  // =========================================================================
  // EscrowFunded  (funded)
  // =========================================================================
  describe("EscrowFunded (funded)", () => {
    it("extracts invoice_id and investor from topics and data fields", () => {
      const event = mkEvent(
        "funded",
        {
          amount: "50000000",
          funded_amount: "150000000",
          status: 1,
          investor_effective_yield_bps: "900",
        },
        ["INV001", "GINVESTOR"],
      );
      const parsed = parseEvent(event) as EscrowFundedEvent;
      expect(parsed.type).toBe("EscrowFunded");
      expect(parsed.name).toBe("funded");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.investor).toBe("GINVESTOR");
      expect(parsed.data.amount).toBe("50000000");
      expect(parsed.data.funded_amount).toBe("150000000");
      expect(parsed.data.status).toBe(1);
      expect(parsed.data.investor_effective_yield_bps).toBe("900");
    });

    it("coerces numeric i128 to string", () => {
      const event = mkEvent(
        "funded",
        { amount: 50000000, funded_amount: 150000000, status: 1, investor_effective_yield_bps: 900 },
        ["INV001", "GINVESTOR"],
      );
      const parsed = parseEvent(event) as EscrowFundedEvent;
      expect(parsed.data.amount).toBe("50000000");
      expect(parsed.data.investor_effective_yield_bps).toBe("900");
    });
  });

  // =========================================================================
  // EscrowSettled  (escrow_sd)
  // =========================================================================
  describe("EscrowSettled (escrow_sd)", () => {
    it("maps all settlement fields", () => {
      const event = mkEvent(
        "escrow_sd",
        {
          funded_amount: "1000000000",
          yield_bps: "800",
          maturity: "1700000000",
          settled_at_ledger_timestamp: "1700001000",
        },
        ["INV001"],
      );
      const parsed = parseEvent(event) as EscrowSettledEvent;
      expect(parsed.type).toBe("EscrowSettled");
      expect(parsed.name).toBe("escrow_sd");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.funded_amount).toBe("1000000000");
      expect(parsed.data.yield_bps).toBe("800");
      expect(parsed.data.maturity).toBe("1700000000");
      expect(parsed.data.settled_at_ledger_timestamp).toBe("1700001000");
    });
  });

  // =========================================================================
  // MaturityUpdated  (maturity)
  // =========================================================================
  describe("MaturityUpdated (maturity)", () => {
    it("maps old and new maturity timestamps", () => {
      const event = mkEvent(
        "maturity",
        { old_maturity: "1700000000", new_maturity: "1710000000" },
        ["INV001"],
      );
      const parsed = parseEvent(event) as MaturityUpdatedEvent;
      expect(parsed.type).toBe("MaturityUpdated");
      expect(parsed.name).toBe("maturity");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.old_maturity).toBe("1700000000");
      expect(parsed.data.new_maturity).toBe("1710000000");
    });
  });

  // =========================================================================
  // AdminTransferred  (admin)
  // =========================================================================
  describe("AdminTransferred (admin)", () => {
    it("maps new_admin from data", () => {
      const event = mkEvent("admin", { new_admin: "GNEWADMIN" }, ["INV001"]);
      const parsed = parseEvent(event) as AdminTransferredEvent;
      expect(parsed.type).toBe("AdminTransferred");
      expect(parsed.name).toBe("admin");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.new_admin).toBe("GNEWADMIN");
    });
  });

  // =========================================================================
  // AdminProposed  (adm_prop)
  // =========================================================================
  describe("AdminProposed (adm_prop)", () => {
    it("maps current_admin and pending_admin", () => {
      const event = mkEvent(
        "adm_prop",
        { current_admin: "GCURR", pending_admin: "GPENDING" },
        ["INV001"],
      );
      const parsed = parseEvent(event) as AdminProposedEvent;
      expect(parsed.type).toBe("AdminProposed");
      expect(parsed.name).toBe("adm_prop");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.current_admin).toBe("GCURR");
      expect(parsed.data.pending_admin).toBe("GPENDING");
    });
  });

  // =========================================================================
  // BeneficiaryRotated  (ben_rot)
  // =========================================================================
  describe("BeneficiaryRotated (ben_rot)", () => {
    it("maps prior_sme and new_sme", () => {
      const event = mkEvent(
        "ben_rot",
        { prior_sme: "GOLD", new_sme: "GNEW" },
        ["INV001"],
      );
      const parsed = parseEvent(event) as BeneficiaryRotatedEvent;
      expect(parsed.type).toBe("BeneficiaryRotated");
      expect(parsed.name).toBe("ben_rot");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.prior_sme).toBe("GOLD");
      expect(parsed.data.new_sme).toBe("GNEW");
    });
  });

  // =========================================================================
  // FundingTargetUpdated  (fund_tgt)
  // =========================================================================
  describe("FundingTargetUpdated (fund_tgt)", () => {
    it("maps old_target and new_target", () => {
      const event = mkEvent(
        "fund_tgt",
        { old_target: "1000000000", new_target: "2000000000" },
        ["INV001"],
      );
      const parsed = parseEvent(event) as FundingTargetUpdatedEvent;
      expect(parsed.type).toBe("FundingTargetUpdated");
      expect(parsed.name).toBe("fund_tgt");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.old_target).toBe("1000000000");
      expect(parsed.data.new_target).toBe("2000000000");
    });
  });

  // =========================================================================
  // LegalHoldChanged  (legalhld)
  // =========================================================================
  describe("LegalHoldChanged (legalhld)", () => {
    it("maps active=1 when hold is set", () => {
      const event = mkEvent("legalhld", { active: 1 }, ["INV001"]);
      const parsed = parseEvent(event) as LegalHoldChangedEvent;
      expect(parsed.type).toBe("LegalHoldChanged");
      expect(parsed.name).toBe("legalhld");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.active).toBe(1);
    });

    it("maps active=0 when hold is cleared", () => {
      const event = mkEvent("legalhld", { active: 0 }, ["INV001"]);
      const parsed = parseEvent(event) as LegalHoldChangedEvent;
      expect(parsed.data.active).toBe(0);
    });
  });

  // =========================================================================
  // DisputePaused  (disppause)
  // =========================================================================
  describe("DisputePaused (disppause)", () => {
    it("maps all dispute pause fields", () => {
      const event = mkEvent(
        "disppause",
        {
          ticket_id: "TICKET-001",
          action: 1,
          paused_at: "1700000000",
          expires_at: "1700003600",
        },
        ["INV001"],
      );
      const parsed = parseEvent(event) as DisputePausedEvent;
      expect(parsed.type).toBe("DisputePaused");
      expect(parsed.name).toBe("disppause");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.ticket_id).toBe("TICKET-001");
      expect(parsed.data.action).toBe(1);
      expect(parsed.data.paused_at).toBe("1700000000");
      expect(parsed.data.expires_at).toBe("1700003600");
    });
  });

  // =========================================================================
  // DisputeResumed  (disp_res)
  // =========================================================================
  describe("DisputeResumed (disp_res)", () => {
    it("maps Manual resume", () => {
      const event = mkEvent(
        "disp_res",
        { admin: "GADMIN", resumed_by: "Manual", ledger_timestamp: "1700005000" },
        ["INV001"],
      );
      const parsed = parseEvent(event) as DisputeResumedEvent;
      expect(parsed.type).toBe("DisputeResumed");
      expect(parsed.name).toBe("disp_res");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.admin).toBe("GADMIN");
      expect(parsed.data.resumed_by).toBe("Manual");
      expect(parsed.data.ledger_timestamp).toBe("1700005000");
    });

    it("maps AutoExpiry resume", () => {
      const event = mkEvent(
        "disp_res",
        { admin: "GADMIN", resumed_by: "AutoExpiry", ledger_timestamp: "1700005000" },
        ["INV001"],
      );
      const parsed = parseEvent(event) as DisputeResumedEvent;
      expect(parsed.data.resumed_by).toBe("AutoExpiry");
    });
  });

  // =========================================================================
  // LegalHoldProposed  (lh_prop)
  // =========================================================================
  describe("LegalHoldProposed (lh_prop)", () => {
    it("maps admin and expires_at from data", () => {
      const event = mkEvent("lh_prop", { admin: "GADMIN", expires_at: "1700003600" });
      const parsed = parseEvent(event) as LegalHoldProposedEvent;
      expect(parsed.type).toBe("LegalHoldProposed");
      expect(parsed.name).toBe("lh_prop");
      expect(parsed.data.admin).toBe("GADMIN");
      expect(parsed.data.expires_at).toBe("1700003600");
    });
  });

  // =========================================================================
  // CollateralRecorded  (coll_rec)
  // =========================================================================
  describe("CollateralRecorded (coll_rec)", () => {
    it("maps invoice_id, amount, and prior_amount from data", () => {
      const event = mkEvent("coll_rec", {
        invoice_id: "INV001",
        amount: "5000000000",
        prior_amount: "0",
      });
      const parsed = parseEvent(event) as CollateralRecordedEvent;
      expect(parsed.type).toBe("CollateralRecorded");
      expect(parsed.name).toBe("coll_rec");
      expect(parsed.data.invoice_id).toBe("INV001");
      expect(parsed.data.amount).toBe("5000000000");
      expect(parsed.data.prior_amount).toBe("0");
    });
  });

  // =========================================================================
  // SmeWithdrew  (sme_wd)
  // =========================================================================
  describe("SmeWithdrew (sme_wd)", () => {
    it("maps amount and recipient", () => {
      const event = mkEvent(
        "sme_wd",
        { amount: "1000000000", recipient: "GSME" },
        ["INV001"],
      );
      const parsed = parseEvent(event) as SmeWithdrewEvent;
      expect(parsed.type).toBe("SmeWithdrew");
      expect(parsed.name).toBe("sme_wd");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.amount).toBe("1000000000");
      expect(parsed.data.recipient).toBe("GSME");
    });
  });

  // =========================================================================
  // InvestorPayoutClaimed  (inv_claim)
  // =========================================================================
  describe("InvestorPayoutClaimed (inv_claim)", () => {
    it("extracts investor from topics[2] and invoice_id from topics[3]; data is empty", () => {
      const event = mkEvent("inv_claim", {}, ["GINVESTOR", "INV001"]);
      const parsed = parseEvent(event) as InvestorPayoutClaimedEvent;
      expect(parsed.type).toBe("InvestorPayoutClaimed");
      expect(parsed.name).toBe("inv_claim");
      expect(parsed.investor).toBe("GINVESTOR");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data).toEqual({});
    });
  });

  // =========================================================================
  // FundingCancelled  (fund_can)
  // =========================================================================
  describe("FundingCancelled (fund_can)", () => {
    it("maps funded_amount", () => {
      const event = mkEvent("fund_can", { funded_amount: "50000000" }, ["INV001"]);
      const parsed = parseEvent(event) as FundingCancelledEvent;
      expect(parsed.type).toBe("FundingCancelled");
      expect(parsed.name).toBe("fund_can");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.funded_amount).toBe("50000000");
    });
  });

  // =========================================================================
  // InvestorRefunded  (refunded)
  // =========================================================================
  describe("InvestorRefunded (refunded)", () => {
    it("extracts investor from topics[2], invoice_id from topics[3], and amount from data", () => {
      const event = mkEvent("refunded", { amount: "50000000" }, ["GINVESTOR", "INV001"]);
      const parsed = parseEvent(event) as InvestorRefundedEvent;
      expect(parsed.type).toBe("InvestorRefunded");
      expect(parsed.name).toBe("refunded");
      expect(parsed.investor).toBe("GINVESTOR");
      expect(parsed.invoice_id).toBe("INV001");
      expect(parsed.data.amount).toBe("50000000");
    });
  });

  // =========================================================================
  // TreasuryDustSwept  (dust_sw)
  // =========================================================================
  describe("TreasuryDustSwept (dust_sw)", () => {
    it("maps invoice_id, token, and amount from data", () => {
      const event = mkEvent("dust_sw", {
        invoice_id: "INV001",
        token: "CTOKEN",
        amount: "1000000",
      });
      const parsed = parseEvent(event) as TreasuryDustSweptEvent;
      expect(parsed.type).toBe("TreasuryDustSwept");
      expect(parsed.name).toBe("dust_sw");
      expect(parsed.data.invoice_id).toBe("INV001");
      expect(parsed.data.token).toBe("CTOKEN");
      expect(parsed.data.amount).toBe("1000000");
    });
  });

  // =========================================================================
  // PrimaryAttestationBound  (att_bind)
  // =========================================================================
  describe("PrimaryAttestationBound (att_bind)", () => {
    it("maps invoice_id and digest", () => {
      const digest = "a".repeat(64);
      const event = mkEvent("att_bind", { invoice_id: "INV001", digest });
      const parsed = parseEvent(event) as PrimaryAttestationBoundEvent;
      expect(parsed.type).toBe("PrimaryAttestationBound");
      expect(parsed.name).toBe("att_bind");
      expect(parsed.data.invoice_id).toBe("INV001");
      expect(parsed.data.digest).toBe(digest);
    });
  });

  // =========================================================================
  // AttestationBound  (att_bound)
  // =========================================================================
  describe("AttestationBound (att_bound)", () => {
    it("maps hash and ledger_timestamp", () => {
      const hash = "b".repeat(64);
      const event = mkEvent("att_bound", { hash, ledger_timestamp: "1700000000" });
      const parsed = parseEvent(event) as AttestationBoundEvent;
      expect(parsed.type).toBe("AttestationBound");
      expect(parsed.name).toBe("att_bound");
      expect(parsed.data.hash).toBe(hash);
      expect(parsed.data.ledger_timestamp).toBe("1700000000");
    });
  });

  // =========================================================================
  // AttestationDigestAppended  (att_app)
  // =========================================================================
  describe("AttestationDigestAppended (att_app)", () => {
    it("maps invoice_id, index, and digest", () => {
      const digest = "c".repeat(64);
      const event = mkEvent("att_app", {
        invoice_id: "INV001",
        index: 3,
        digest,
      });
      const parsed = parseEvent(event) as AttestationDigestAppendedEvent;
      expect(parsed.type).toBe("AttestationDigestAppended");
      expect(parsed.name).toBe("att_app");
      expect(parsed.data.invoice_id).toBe("INV001");
      expect(parsed.data.index).toBe(3);
      expect(parsed.data.digest).toBe(digest);
    });
  });

  // =========================================================================
  // AllowlistEnabledChanged  (al_ena)
  // =========================================================================
  describe("AllowlistEnabledChanged (al_ena)", () => {
    it("maps invoice_id and active=1", () => {
      const event = mkEvent("al_ena", { invoice_id: "INV001", active: 1 });
      const parsed = parseEvent(event) as AllowlistEnabledChangedEvent;
      expect(parsed.type).toBe("AllowlistEnabledChanged");
      expect(parsed.name).toBe("al_ena");
      expect(parsed.data.invoice_id).toBe("INV001");
      expect(parsed.data.active).toBe(1);
    });

    it("maps active=0 for disabled", () => {
      const event = mkEvent("al_ena", { invoice_id: "INV001", active: 0 });
      const parsed = parseEvent(event) as AllowlistEnabledChangedEvent;
      expect(parsed.data.active).toBe(0);
    });
  });

  // =========================================================================
  // InvestorAllowlistChanged  (al_set)
  // =========================================================================
  describe("InvestorAllowlistChanged (al_set)", () => {
    it("maps invoice_id, investor, and allowed=1", () => {
      const event = mkEvent("al_set", {
        invoice_id: "INV001",
        investor: "GINVESTOR",
        allowed: 1,
      });
      const parsed = parseEvent(event) as InvestorAllowlistChangedEvent;
      expect(parsed.type).toBe("InvestorAllowlistChanged");
      expect(parsed.name).toBe("al_set");
      expect(parsed.data.invoice_id).toBe("INV001");
      expect(parsed.data.investor).toBe("GINVESTOR");
      expect(parsed.data.allowed).toBe(1);
    });

    it("maps allowed=0 for blocked", () => {
      const event = mkEvent("al_set", {
        invoice_id: "INV001",
        investor: "GINVESTOR",
        allowed: 0,
      });
      const parsed = parseEvent(event) as InvestorAllowlistChangedEvent;
      expect(parsed.data.allowed).toBe(0);
    });
  });

  // =========================================================================
  // Unknown fallback
  // =========================================================================
  describe("Unknown event fallback", () => {
    it("returns UnknownEvent for an unrecognised symbol", () => {
      const event = mkEvent("future_event", { some_field: 42 });
      const parsed = parseEvent(event) as UnknownEvent;
      expect(parsed.type).toBe("Unknown");
      expect(parsed.name).toBe("future_event");
      expect(parsed.raw).toBe(event);
    });

    it("returns UnknownEvent when topics array is too short", () => {
      const event: EscrowEvent = {
        id: "short",
        type: "contract",
        contract_id: "CESCROW",
        ledger: 1,
        ledger_closed_at: "",
        paging_token: "1-0",
        topics: ["only_one_topic"],
        value: {},
      };
      const parsed = parseEvent(event) as UnknownEvent;
      expect(parsed.type).toBe("Unknown");
      expect(parsed.name).toBe("");
    });

    it("returns UnknownEvent when topics[1] is not a string", () => {
      const event: EscrowEvent = {
        id: "bad-topic",
        type: "contract",
        contract_id: "CESCROW",
        ledger: 1,
        ledger_closed_at: "",
        paging_token: "1-0",
        topics: ["first", 42],
        value: {},
      };
      const parsed = parseEvent(event) as UnknownEvent;
      expect(parsed.type).toBe("Unknown");
      expect(parsed.name).toBe("");
    });
  });

  // =========================================================================
  // _map envelope compatibility
  // =========================================================================
  describe("_map envelope compatibility", () => {
    it("reads fields from a _map-style data payload", () => {
      const event: EscrowEvent = {
        id: "map-test",
        type: "contract",
        contract_id: "CESCROW",
        ledger: 1,
        ledger_closed_at: "",
        paging_token: "1-0",
        topics: ["fund_can_topic", "fund_can", "INV001"],
        value: {
          _map: [
            ["funded_amount", "99000000"],
          ],
        },
      };
      const parsed = parseEvent(event) as FundingCancelledEvent;
      expect(parsed.type).toBe("FundingCancelled");
      expect(parsed.data.funded_amount).toBe("99000000");
    });
  });
});
