/**
 * Demonstrates the yield claim delegation flow with an RPC adapter stub.
 *
 * A real adapter must authorize set/revoke calls with the investor and claim
 * calls with the delegate. The escrow must already be funded and claimable.
 */

import { EscrowClient, type SorobanRpcClient } from "../src";

const investor = "GINVESTOR...";
const delegate = "GDELEGATE...";

const rpc: SorobanRpcClient = {
  async invoke(_contractId, functionName, args) {
    console.log(`${functionName}(${JSON.stringify(args)})`);
  },
  async simulate() {
    throw new Error("This example does not perform read-only calls");
  },
  async getLedger() {
    return { timestamp: Math.floor(Date.now() / 1000), sequence: 1 };
  },
};

async function main(): Promise<void> {
  const client = new EscrowClient(
    {
      rpcUrl: "https://soroban-testnet.stellar.org",
      networkPassphrase: "Test SDF Network ; September 2015",
      contractId: "CCONTRACT...",
    },
    rpc,
  );

  await client.setYieldClaimDelegate(investor, delegate);
  await client.claimPayoutAsDelegate(investor, delegate);
  await client.revokeYieldClaimDelegate(investor);
}

main().catch((error: unknown) => {
  console.error("Yield claim delegation example failed:", error);
  process.exitCode = 1;
});
