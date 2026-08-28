/**
 * karis-ky Escrow SDK — Example Usage
 *
 * Demonstrates the typed client wrapper for common escrow lifecycle operations.
 *
 * To run: npx ts-node examples/basic-usage.ts
 */

import {
  EscrowClient,
  EscrowStatus,
  EscrowErrorCode,
  ESCROW_STATUS_LABELS,
  ESCROW_ERROR_LABELS,
  classifyError,
  toBaseUnits,
  fromBaseUnits,
  type InvoiceEscrow,
  type InitParams,
  type SorobanRpcClient,
} from "../src";

// ---------------------------------------------------------------------------
// Mock RPC client for demonstration purposes.
// Replace with a real @stellar/stellar-sdk SorobanServer in production.
// ---------------------------------------------------------------------------

class MockRpcClient implements SorobanRpcClient {
  private ledger = { timestamp: Math.floor(Date.now() / 1000), sequence: 1 };
  private state: Record<string, unknown> = {};

  async invoke(contractId: string, fnName: string, args: unknown[], _source?: string): Promise<unknown> {
    console.log(`  [invoke] ${fnName}(${JSON.stringify(args)})`);
    return this.handleInvoke(fnName, args);
  }

  async simulate(contractId: string, fnName: string, args: unknown[]): Promise<unknown> {
    return this.handleInvoke(fnName, args);
  }

  async getLedger(): Promise<{ timestamp: number; sequence: number }> {
    return { ...this.ledger };
  }

  private handleInvoke(fnName: string, args: unknown[]): unknown {
    switch (fnName) {
      case "get_version":
        return 6;
      case "get_interface_version":
        return 1;
      case "get_escrow":
        return this.state["escrow"] || null;
      case "init":
        this.state["escrow"] = {
          invoice_id: (args as string[])[1],
          admin: (args as string[])[0],
          sme_address: (args as string[])[2],
          amount: (args as string[])[3],
          funding_target: (args as string[])[3],
          funded_amount: "0",
          yield_bps: (args as string[])[4],
          maturity: (args as string[])[5],
          status: 0,
        };
        return this.state["escrow"];
      case "fund":
        this.state["funded"] = "5000_0000000";
        return {
          ...(this.state["escrow"] as object),
          funded_amount: "5000_0000000",
          status: 0,
        };
      case "get_legal_hold":
        return false;
      default:
        return null;
    }
  }
}

// ---------------------------------------------------------------------------
// Main example flow
// ---------------------------------------------------------------------------

async function main() {
  console.log("=== karis-ky Escrow SDK — Example Usage ===\n");

  // 1. Create the client
  const mockRpc = new MockRpcClient();
  const client = new EscrowClient(
    {
      rpcUrl: "http://localhost:8000/soroban/rpc",
      networkPassphrase: "Standalone Network ; February 2017",
      contractId: "CCONTXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
      specUrl: "https://spec.karis-ky.com/escrow/spec.json",
    },
    mockRpc,
  );

  console.log(`Connected to: ${client.getRpcUrl()}`);
  console.log(`Contract ID:  ${client.getContractId()}\n`);

  // 2. Validate interface version
  console.log("→ Validating interface version...");
  try {
    await client.validateInterfaceVersion();
    console.log("  ✓ Interface version matches SDK.\n");
  } catch (e) {
    console.error(`  ✗ ${e}\n`);
  }

  // 3. Read schema version
  const version = await client.getVersion();
  console.log(`→ Schema version: ${version}\n`);

  // 4. Check legal hold
  const hold = await client.getLegalHold();
  console.log(`→ Legal hold active: ${hold}\n`);

  // 5. Initialize an escrow
  console.log("→ Initializing escrow...");
  const initParams: InitParams = {
    admin: "GADMINXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
    invoice_id: "INV001",
    sme_address: "GSMEXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
    amount: toBaseUnits("10000"), // 10,000 tokens with 7 decimals → "100000000000"
    yield_bps: "800", // 8% annualised
    maturity: "0", // No maturity gate
    funding_token: "CTOKENXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
    registry: null,
    treasury: "GTREAXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
    yield_tiers: null,
    min_contribution: null,
    max_unique_investors: null,
    max_per_investor: null,
    legal_hold_clear_delay: null,
    funding_deadline: null,
    yield_slippage_threshold: null,
  };

  const escrow = await client.init(initParams);
  console.log(`  Created invoice: ${escrow.invoice_id}`);
  console.log(`  Status: ${ESCROW_STATUS_LABELS[escrow.status]}`);
  console.log(`  Target: ${fromBaseUnits(escrow.funding_target)} tokens\n`);

  // 6. Fund the escrow
  console.log("→ Funding escrow (5,000 tokens from Investor 1)...");
  const fundedAmount = toBaseUnits("5000");
  const updatedEscrow = await client.fund(
    "GINV1XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
    fundedAmount,
  );
  console.log(`  Funded amount: ${fromBaseUnits(updatedEscrow.funded_amount)} tokens`);
  console.log(`  Status: ${ESCROW_STATUS_LABELS[updatedEscrow.status]}\n`);

  // 7. Error classification demo
  console.log("→ Error code classification demo:");
  const testCodes = [3, 103, 122, 164];
  for (const code of testCodes) {
    const category = classifyError(code);
    const label = ESCROW_ERROR_LABELS[code] || "Unknown";
    console.log(`  Code ${code} (${label}) → category: "${category}"`);
  }

  console.log("\n=== Example complete ===");
}

main().catch(console.error);
