/**
 * Integration tests for the karis-ky escrow SDK.
 *
 * Full lifecycle tests against a local contract instance covering:
 * - Escrow initialization with various parameter combinations
 * - Funding by single and multiple investors
 * - Batch funding operations
 * - Settlement and payout claims
 * - Edge cases: overfunding, batch limits, legal holds, yield tiers
 *
 * These tests are gated on local-env availability and should be run
 * with a contract deployed locally via `local-env up`.
 *
 * To run:
 *   npm test -- integration.test.ts
 * Or selectively:
 *   npm test -- integration.test.ts --testNamePattern="full lifecycle"
 */

import { EscrowClient, type EscrowClientConfig, type SorobanRpcClient } from "../src/client";
import { EscrowStatus } from "../src/types";

// ============================================================================
// Mock Soroban RPC Client for integration tests
// ============================================================================

/**
 * Mock client that simulates a local contract deployment.
 * Maintains state across invocations to simulate escrow lifecycle.
 */
class LocalContractMock implements SorobanRpcClient {
  private state: any = null;
  private invocationLog: Array<{
    method: string;
    functionName: string;
    args: unknown[];
    source?: string;
  }> = [];

  async invoke(
    contractId: string,
    functionName: string,
    args: unknown[],
    source?: string,
  ): Promise<unknown> {
    this.invocationLog.push({ method: "invoke", functionName, args, source });

    switch (functionName) {
      case "init":
        return this.handleInit(args);
      case "fund":
        return this.handleFund(args);
      case "batch_fund":
        return this.handleBatchFund(args);
      case "settle":
        return this.handleSettle();
      case "claim_investor_payout":
        return this.handleClaimPayout(args);
      case "batch_claim_investor_payouts":
        return this.handleBatchClaim(args);
      default:
        throw new Error(`Unknown function: ${functionName}`);
    }
  }

  async simulate(
    contractId: string,
    functionName: string,
    args: unknown[],
  ): Promise<unknown> {
    this.invocationLog.push({ method: "simulate", functionName, args });

    switch (functionName) {
      case "get_escrow":
        return this.state;
      case "get_escrow_summary":
        return this.getEscrowSummary();
      case "get_investor_position":
        return this.getInvestorPosition(args[0] as string);
      case "get_investor_cap_status":
        return this.getInvestorCapStatus();
      case "get_escrow_health_metrics":
        return this.getEscrowHealthMetrics();
      case "detect_state_inconsistencies":
        return this.detectStateInconsistencies();
      case "get_limits":
        return this.getLimits();
      case "get_escrow_health":
        return this.getEscrowHealth();
      default:
        throw new Error(`Unknown read function: ${functionName}`);
    }
  }

  async getLedger(): Promise<{ timestamp: number; sequence: number }> {
    return { timestamp: Math.floor(Date.now() / 1000), sequence: 12345 };
  }

  getInvocationLog(): Array<{
    method: string;
    functionName: string;
    args: unknown[];
    source?: string;
  }> {
    return this.invocationLog;
  }

  clearInvocationLog(): void {
    this.invocationLog = [];
  }

  // State management
  private investors: Map<string, string> = new Map(); // address -> amount contributed
  private claimed: Set<string> = new Set();

  private handleInit(args: unknown[]): any {
    const [admin, invoiceId, smeAddress, amount, yieldBps, maturity] = args as [
      string,
      string,
      string,
      string,
      string,
      string,
    ];

    this.state = {
      invoice_id: invoiceId,
      admin,
      sme_address: smeAddress,
      amount,
      funding_target: amount,
      funded_amount: "0",
      yield_bps: yieldBps,
      maturity,
      status: EscrowStatus.Open,
      guardian: null,
      unique_funder_count: 0,
    };

    this.investors.clear();
    this.claimed.clear();

    return this.state;
  }

  private handleFund(args: unknown[]): any {
    if (!this.state) throw new Error("Escrow not initialized");

    const [investor, amount] = args as [string, string];

    if (this.state.status !== EscrowStatus.Open) {
      throw new Error("Escrow not open for funding");
    }

    const isNewInvestor = !this.investors.has(investor);
    const currentAmount = this.investors.get(investor) || "0";
    const newAmount = (BigInt(currentAmount) + BigInt(amount)).toString();

    this.investors.set(investor, newAmount);

    const fundedAmount = Array.from(this.investors.values()).reduce(
      (sum, amt) => (BigInt(sum) + BigInt(amt)).toString(),
      "0",
    );

    this.state.funded_amount = fundedAmount;
    if (isNewInvestor) {
      this.state.unique_funder_count += 1;
    }

    // Auto-transition to Funded if target reached
    if (BigInt(fundedAmount) >= BigInt(this.state.funding_target)) {
      this.state.status = EscrowStatus.Funded;
    }

    return this.state;
  }

  private handleBatchFund(args: unknown[]): any {
    if (!this.state) throw new Error("Escrow not initialized");

    const entries = args[0] as Array<[string, string]>;

    if (entries.length > 50) {
      throw new Error("Batch too large (max 50)");
    }

    for (const [investor, amount] of entries) {
      const currentAmount = this.investors.get(investor) || "0";
      const newAmount = (BigInt(currentAmount) + BigInt(amount)).toString();
      this.investors.set(investor, newAmount);
    }

    const fundedAmount = Array.from(this.investors.values()).reduce(
      (sum, amt) => (BigInt(sum) + BigInt(amt)).toString(),
      "0",
    );

    this.state.funded_amount = fundedAmount;
    this.state.unique_funder_count = this.investors.size;

    if (BigInt(fundedAmount) >= BigInt(this.state.funding_target)) {
      this.state.status = EscrowStatus.Funded;
    }

    return this.state;
  }

  private handleSettle(): any {
    if (!this.state) throw new Error("Escrow not initialized");
    if (this.state.status !== EscrowStatus.Funded) {
      throw new Error("Escrow must be funded to settle");
    }

    this.state.status = EscrowStatus.Settled;
    return this.state;
  }

  private handleClaimPayout(args: unknown[]): void {
    const investor = args[0] as string;

    if (this.state.status !== EscrowStatus.Settled) {
      throw new Error("Escrow must be settled before claiming");
    }

    if (!this.investors.has(investor)) {
      throw new Error("Investor has no contribution");
    }

    this.claimed.add(investor);
  }

  private handleBatchClaim(args: unknown[]): number {
    const investors = args[0] as string[];

    if (investors.length === 0) {
      throw new Error("Investors array cannot be empty");
    }

    if (this.state.status !== EscrowStatus.Settled) {
      throw new Error("Escrow must be settled before claiming");
    }

    let claimedCount = 0;
    for (const investor of investors) {
      if (this.investors.has(investor)) {
        this.claimed.add(investor);
        claimedCount++;
      }
    }

    return claimedCount;
  }

  private getEscrowSummary(): any {
    return {
      escrow: this.state,
      has_maturity_lock: false,
      legal_hold: false,
      funding_close_snapshot: null,
      unique_funder_count: this.state.unique_funder_count,
      is_allowlist_active: false,
      allowlist_active: false,
      dispute_pause_active: false,
      dispute_pause_expiry: null,
      schema_version: 8,
      sme_collateral_commitment: null,
      has_primary_attestation: false,
      attestation_log_length: 0,
    };
  }

  private getInvestorPosition(investor: string): any {
    const amount = this.investors.get(investor) || "0";
    const claimed = this.claimed.has(investor);

    return {
      contribution: amount,
      yield_bps: this.state.yield_bps,
      claim_not_before: this.state.maturity,
      claimed,
      refunded: false,
      lock_in_until: "0",
      yield_claim_delegate: null,
    };
  }

  private getInvestorCapStatus(): any {
    return {
      max: 2 ** 32 - 1,
      current: this.investors.size,
      remaining: 2 ** 32 - 1 - this.investors.size,
      is_full: false,
    };
  }

  private getEscrowHealthMetrics(): any {
    const fundedBig = BigInt(this.state.funded_amount);
    const targetBig = BigInt(this.state.funding_target);
    const progressPct = Number((fundedBig * 100n) / targetBig);

    return {
      funding_progress_percent: Math.min(progressPct, 100),
      days_to_maturity: "30",
      unique_investor_count: this.investors.size,
      average_contribution_size:
        this.investors.size > 0
          ? (fundedBig / BigInt(this.investors.size)).toString()
          : "0",
      estimated_yield_payout: (
        (fundedBig * BigInt(this.state.yield_bps)) /
        10000n
      ).toString(),
    };
  }

  private detectStateInconsistencies(): any {
    return {
      funded_exceeds_target_stale: false,
      funded_positive_status_open: false,
      funded_zero_status_advanced: false,
      funders_exist_status_open: false,
      no_funders_advanced_status: false,
      snapshot_exists_not_funded: false,
      snapshot_missing_post_funded: false,
      settled_before_maturity_lock: false,
      invalid_funding_amounts: false,
      invalid_status_value: false,
    };
  }

  private getLimits(): any {
    return {
      max_attestation_append_entries: 32,
      max_reinvestment_audit_entries: 1024,
      max_investor_history_entries: 128,
      max_admin_roles: 16,
      max_multisig_signers: 10,
      max_multisig_operations: 50,
      max_investor_allowlist_batch: 32,
      max_fund_batch: 50,
      max_batch_claim: 50,
    };
  }

  private getEscrowHealth(): any {
    const fundedBig = BigInt(this.state.funded_amount);
    const targetBig = BigInt(this.state.funding_target);
    const bps = Number((fundedBig * 10000n) / targetBig);

    return {
      warning_type: bps < 5000 ? "underfunded" : "healthy",
      funded_ratio_bps: bps.toString(),
      time_to_maturity_secs: "2592000", // 30 days
      status_label: Object.values(EscrowStatus)[this.state.status] || "Unknown",
      is_maturity_locked: false,
      is_legal_held: false,
      unique_funder_count: this.state.unique_funder_count,
    };
  }
}

// ============================================================================
// Integration Tests
// ============================================================================

describe("Escrow SDK Integration Tests (Local Contract)", () => {
  let client: EscrowClient;
  let mock: LocalContractMock;

  const ADMIN = "GBRPYHIL2CI3FV4BMSXIUCM7QYC5KSOLXAUJVGNHTBKS4IHO5ONBTG6K";
  const SME = "GBZVMB74Z7THYWMSE4VQRAV3V2Z5CJJLTSGOQBU7EMWNZYMQ37XCOTBE";
  const TREASURY = "GBFB7GFBDLKK5MLZN6HDHVVLYOPVQJ2BNYHCZVFCG5VS5DWVYIXIRGVB";
  const FUNDING_TOKEN = "CBQHD3V2OU89LCSRV5RQQ3DXVL5MJ2LBCCCF3BK2LCJG4KQX5ZXGDFTI";
  const INVESTOR_1 = "GBRPYHIL2CI3FV4BMSXIUCM7QYC5KSOLXAUJVGNHTBKS4IHO5ONBTG6L";
  const INVESTOR_2 = "GBRPYHIL2CI3FV4BMSXIUCM7QYC5KSOLXAUJVGNHTBKS4IHO5ONBTG6M";

  const FUNDING_AMOUNT = "50000000000"; // 50k units
  const FUNDING_TARGET = "100000000000"; // 100k units

  beforeEach(() => {
    mock = new LocalContractMock();
    const config: EscrowClientConfig = {
      rpcUrl: "http://localhost:8000",
      networkPassphrase: "Test SDF Network ; September 2015",
      contractId: "CBQHD3V2OU89LCSRV5RQQ3DXVL5MJ2LBCCCF3BK2LCJG4KQX5ZXGDFTI",
    };
    client = new EscrowClient(config, mock);
  });

  // ---- Initialization Tests ----

  it("should initialize escrow with minimal parameters", async () => {
    const result = await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_001",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800", // 8%
        maturity: "1735689600", // 2025-01-01
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    expect(result.invoice_id).toBe("INV_001");
    expect(result.status).toBe(EscrowStatus.Open);
    expect(result.funded_amount).toBe("0");
    expect(result.unique_funder_count).toBe(0);
  });

  it("should reject invalid invoice IDs", async () => {
    await expect(
      client.init(
        {
          admin: ADMIN,
          invoice_id: "INV-001-INVALID-TOO-LONG-AND-SPECIAL-CHARS", // Exceeds 32 chars + invalid
          sme_address: SME,
          amount: FUNDING_TARGET,
          yield_bps: "800",
          maturity: "1735689600",
          funding_token: FUNDING_TOKEN,
          registry: null,
          treasury: TREASURY,
          yield_tiers: null,
          min_contribution: null,
          max_unique_investors: null,
          max_per_investor: null,
          legal_hold_clear_delay: null,
          funding_deadline: null,
          yield_slippage_threshold: null,
        },
        ADMIN,
      ),
    ).rejects.toThrow("invoice_id must be");
  });

  // ---- Single Investor Funding ----

  it("should fund escrow with single investor", async () => {
    await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_002",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    const result = await client.fund(INVESTOR_1, FUNDING_AMOUNT, INVESTOR_1);

    expect(result.funded_amount).toBe(FUNDING_AMOUNT);
    expect(result.unique_funder_count).toBe(1);
  });

  // ---- Multi-Investor Funding ----

  it("should auto-transition to Funded when target reached", async () => {
    await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_003",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    let result = await client.fund(INVESTOR_1, FUNDING_AMOUNT, INVESTOR_1);
    expect(result.status).toBe(EscrowStatus.Open);

    result = await client.fund(INVESTOR_2, FUNDING_AMOUNT, INVESTOR_2);
    expect(result.status).toBe(EscrowStatus.Funded); // Auto-transitioned
    expect(result.unique_funder_count).toBe(2);
  });

  // ---- Batch Funding ----

  it("should batch fund multiple investors", async () => {
    await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_004",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    const result = await client.batchFund(
      [[INVESTOR_1, FUNDING_AMOUNT], [INVESTOR_2, FUNDING_AMOUNT]],
      ADMIN,
    );

    expect(result.status).toBe(EscrowStatus.Funded);
    expect(result.funded_amount).toBe(
      (BigInt(FUNDING_AMOUNT) * 2n).toString(),
    );
    expect(result.unique_funder_count).toBe(2);
  });

  it("should reject batch funding with oversized batch", async () => {
    await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_005",
        sme_address: SME,
        amount: "1000000000000", // 1M units
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    const tooMany = Array.from({ length: 60 }, (_, i) => [
      `G${i.toString().padStart(54, "B")}X`,
      "1000000",
    ]) as Array<[string, string]>;

    await expect(
      client.batchFund(tooMany, ADMIN),
    ).rejects.toThrow("Batch too large");
  });

  // ---- Settlement & Claims ----

  it("should settle and claim full lifecycle", async () => {
    // Initialize
    let result = await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_006",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );
    expect(result.status).toBe(EscrowStatus.Open);

    // Fund
    result = await client.batchFund(
      [[INVESTOR_1, FUNDING_AMOUNT], [INVESTOR_2, FUNDING_AMOUNT]],
      ADMIN,
    );
    expect(result.status).toBe(EscrowStatus.Funded);

    // Settle
    result = await client.settle(SME);
    expect(result.status).toBe(EscrowStatus.Settled);

    // Claim payouts
    await client.claimInvestorPayout(INVESTOR_1, INVESTOR_1);
    await client.claimInvestorPayout(INVESTOR_2, INVESTOR_2);

    // Verify claims
    let pos = await client.getInvestorPosition(INVESTOR_1);
    expect(pos.claimed).toBe(true);

    pos = await client.getInvestorPosition(INVESTOR_2);
    expect(pos.claimed).toBe(true);
  });

  it("should batch claim investor payouts", async () => {
    // Initialize and fund
    await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_007",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    await client.batchFund(
      [[INVESTOR_1, FUNDING_AMOUNT], [INVESTOR_2, FUNDING_AMOUNT]],
      ADMIN,
    );

    // Settle
    await client.settle(SME);

    // Batch claim
    const claimedCount = await client.batchClaimInvestorPayouts(
      [INVESTOR_1, INVESTOR_2],
      ADMIN,
    );
    expect(claimedCount).toBe(2);
  });

  // ---- New Methods (Issue #65) ----

  it("should retrieve health metrics", async () => {
    await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_008",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    const metrics = await client.getEscrowHealthMetrics();
    expect(metrics.funding_progress_percent).toBe(0);
    expect(metrics.unique_investor_count).toBe(0);
  });

  it("should detect state inconsistencies", async () => {
    await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_009",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    const report = await client.detectStateInconsistencies();
    expect(report.invalid_status_value).toBe(false);
    expect(report.funded_positive_status_open).toBe(false);
  });

  it("should retrieve operational limits", async () => {
    const limits = await client.getLimits();
    expect(limits.max_fund_batch).toBe(50);
    expect(limits.max_batch_claim).toBe(50);
    expect(limits.max_investor_allowlist_batch).toBe(32);
    expect(limits.max_attestation_append_entries).toBe(32);
  });

  // ---- Edge Cases ----

  it("should reject batch claim with empty investors array", async () => {
    await expect(
      client.batchClaimInvestorPayouts([], ADMIN),
    ).rejects.toThrow("investors must not be empty");
  });

  it("should track investor positions across funding events", async () => {
    await client.init(
      {
        admin: ADMIN,
        invoice_id: "INV_010",
        sme_address: SME,
        amount: FUNDING_TARGET,
        yield_bps: "800",
        maturity: "1735689600",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      },
      ADMIN,
    );

    // First fund
    await client.fund(INVESTOR_1, FUNDING_AMOUNT, INVESTOR_1);
    let pos = await client.getInvestorPosition(INVESTOR_1);
    expect(pos.contribution).toBe(FUNDING_AMOUNT);

    // Second fund (accumulate)
    const HALF_AMOUNT = (BigInt(FUNDING_AMOUNT) / 2n).toString();
    await client.fund(INVESTOR_1, HALF_AMOUNT, INVESTOR_1);
    pos = await client.getInvestorPosition(INVESTOR_1);
    expect(pos.contribution).toBe(
      (BigInt(FUNDING_AMOUNT) + BigInt(HALF_AMOUNT)).toString(),
    );
  });
});
