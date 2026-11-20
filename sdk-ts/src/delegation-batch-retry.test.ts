import { EscrowClient, type SorobanRpcClient } from "./client";
import { filterFundingBatchForRetry } from "./batch-retry";

describe("EscrowClient yield claim delegation methods", () => {
  it("invokes all three delegation entrypoints with typed arguments", async () => {
    const invoke = jest.fn().mockResolvedValue(undefined);
    const rpc: SorobanRpcClient = {
      invoke,
      simulate: jest.fn(),
      getLedger: jest.fn(),
    };
    const client = new EscrowClient(
      { rpcUrl: "https://rpc.example", networkPassphrase: "test", contractId: "contract" },
      rpc,
    );

    await client.setYieldClaimDelegate("investor", "delegate", "investor");
    await client.claimPayoutAsDelegate("investor", "delegate", "delegate");
    await client.revokeYieldClaimDelegate("investor", "investor");

    expect(invoke.mock.calls).toEqual([
      ["contract", "set_yield_claim_delegate", ["investor", "delegate"], "investor"],
      ["contract", "claim_payout_as_delegate", ["investor", "delegate"], "delegate"],
      ["contract", "revoke_yield_claim_delegate", ["investor"], "investor"],
    ]);
  });
});

describe("filterFundingBatchForRetry", () => {
  const entries = [
    ["investor-a", "5"],
    ["investor-b", "7"],
  ] as const;

  it("returns the whole batch when no funding changes are present", () => {
    expect(
      filterFundingBatchForRetry(entries, {
        before: new Map([["investor-a", "10"], ["investor-b", "20"]]),
        current: new Map([["investor-a", "10"], ["investor-b", "20"]]),
      }),
    ).toEqual([["investor-a", "5"], ["investor-b", "7"]]);
  });

  it("removes the whole batch when its funding changes are already present", () => {
    expect(
      filterFundingBatchForRetry(entries, {
        before: new Map([["investor-a", "10"], ["investor-b", "20"]]),
        current: new Map([["investor-a", "15"], ["investor-b", "27"]]),
      }),
    ).toEqual([]);
  });

  it("rejects mixed state instead of recommending a partial retry", () => {
    expect(() =>
      filterFundingBatchForRetry(entries, {
        before: new Map([["investor-a", "10"], ["investor-b", "20"]]),
        current: new Map([["investor-a", "15"], ["investor-b", "20"]]),
      }),
    ).toThrow("refusing to retry only part of an atomic batch");
  });

  it("accounts for repeated investors in one batch", () => {
    expect(
      filterFundingBatchForRetry(
        [["investor-a", "5"], ["investor-a", "7"]],
        {
          before: new Map([["investor-a", "10"]]),
          current: new Map([["investor-a", "22"]]),
        },
      ),
    ).toEqual([]);
  });
});
