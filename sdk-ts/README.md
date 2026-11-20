# karis-ky TypeScript SDK

Typed client wrapper for the karis-ky LiquifactEscrow Soroban contract.

## Features

- **Typed client** — All contract entrypoints exposed as async methods with full TypeScript support.
- **Typed event parser** — `parseEvent` maps raw Soroban events to a typed discriminated union covering all 24 event types in `docs/EVENT_SCHEMA.md`.
- **Error codes** — Stable, append-only error codes with human-readable labels and category helpers.
- **Contract spec** — Machine-readable ABI bundled for validation and discovery.
- **Examples** — Complete workflow demonstrations for funding, settlement, and investor claims.

## Quick Start

### Installation

```bash
npm install @karis-ky/escrow-sdk
```

### Basic Usage

```typescript
import { EscrowClient } from "@karis-ky/escrow-sdk";
import { SorobanServer } from "@stellar/stellar-sdk";

// Initialize RPC client
const rpc = new SorobanServer("https://soroban-testnet.stellar.org");

// Create escrow client
const client = new EscrowClient(
  {
    rpcUrl: rpc.serverURL.toString(),
    networkPassphrase: "Test SDF Network ; September 2015",
    contractId: "CCONTRACTID...",
  },
  rpc,
);

// Read escrow state
const escrow = await client.getEscrow();
console.log(`Escrow status: ${escrow.status}`);

// Fund an escrow
await client.fund("GINVESTOR...", "1000000000"); // 100 tokens (7 decimals)

// Settle (requires SME auth)
await client.settle();

// Investor claim payout
await client.claimInvestorPayout("GINVESTOR...");
```

## Typed Event Parser

`parseEvent` turns a raw `EscrowEvent` (as returned by `subscribeEscrowEvents` or any Soroban RPC adapter) into a typed discriminated union. All 24 event types from `docs/EVENT_SCHEMA.md` are covered.

```typescript
import { parseEvent } from "@karis-ky/escrow-sdk";
import type { EscrowFundedEvent, EscrowSettledEvent } from "@karis-ky/escrow-sdk";

// Inside a subscribeEscrowEvents loop:
for await (const rawEvent of client.subscribeEscrowEvents({ start_ledger: 1000 })) {
  const parsed = parseEvent(rawEvent);

  switch (parsed.type) {
    case "EscrowFunded": {
      const e = parsed as EscrowFundedEvent;
      console.log(`${e.invoice_id} funded: +${e.data.amount} by ${e.investor}`);
      break;
    }
    case "EscrowSettled": {
      const e = parsed as EscrowSettledEvent;
      console.log(`${e.invoice_id} settled at ledger ${e.data.settled_at_ledger_timestamp}`);
      break;
    }
    case "Unknown":
      // Forward-compatible: new events do not throw, they fall here.
      break;
  }
}
```

### Routing

`parseEvent` routes on `topics[1]`, the karis-ky short event symbol (e.g. `"funded"`, `"escrow_sd"`). The function never throws: unrecognised symbols return `{ type: "Unknown", name, raw }` so callers remain forward-compatible with new contract events.

### Data field access

Numeric `i128`/`i64`/`u64` values are returned as `string` to preserve precision. `u32` counts and flags (e.g. `active`, `allowed`) are returned as `number`. Both plain-object and `{ _map: [...] }` envelope formats are supported, covering all Soroban RPC adapter variants.

### Supported events

| `type` discriminant | Symbol | Entrypoint |
| --- | --- | --- |
| `EscrowInitialized` | `escrow_ii` | `init` |
| `MaxUniqueInvestorsCapLowered` | `inv_cap` | `lower_max_unique_investors` |
| `EscrowFunded` | `funded` | `fund`, `fund_with_commitment` |
| `EscrowSettled` | `escrow_sd` | `settle` |
| `MaturityUpdated` | `maturity` | `update_maturity` |
| `AdminTransferred` | `admin` | `accept_admin` |
| `AdminProposed` | `adm_prop` | `propose_admin` |
| `BeneficiaryRotated` | `ben_rot` | `rotate_beneficiary` |
| `FundingTargetUpdated` | `fund_tgt` | `update_funding_target` |
| `LegalHoldChanged` | `legalhld` | `set_legal_hold`, `clear_legal_hold` |
| `DisputePaused` | `disppause` | `pause_dispute` |
| `DisputeResumed` | `disp_res` | `resume_dispute` |
| `LegalHoldProposed` | `lh_prop` | `propose_legal_hold` |
| `CollateralRecorded` | `coll_rec` | `record_sme_collateral_commitment` |
| `SmeWithdrew` | `sme_wd` | `withdraw` |
| `InvestorPayoutClaimed` | `inv_claim` | `claim_investor_payout` |
| `FundingCancelled` | `fund_can` | `cancel_funding` |
| `InvestorRefunded` | `refunded` | `refund` |
| `TreasuryDustSwept` | `dust_sw` | `sweep_terminal_dust` |
| `PrimaryAttestationBound` | `att_bind` | `bind_primary_attestation_hash` |
| `AttestationBound` | `att_bound` | `bind_primary_attestation_hash` |
| `AttestationDigestAppended` | `att_app` | `append_attestation_digest` |
| `AllowlistEnabledChanged` | `al_ena` | `set_allowlist_active` |
| `InvestorAllowlistChanged` | `al_set` | `set_investor_allowlisted` |

## Error Handling

All contract errors are typed. See the [Error Handling Guide](docs/error-handling.md) for:

- How to detect and distinguish `ContractError(code)` from network errors
- Error code reference tables for all 40+ error codes
- Common error patterns by method (fund, settle, claim)
- Three complete try/catch examples with recovery logic
- Error recovery strategies

### Quick Example

```typescript
import {
  EscrowErrorCode,
  ESCROW_ERROR_LABELS,
  classifyError,
  SorobanContractError,
} from "@karis-ky/escrow-sdk";

try {
  await client.fund(investor, amount);
} catch (err: unknown) {
  if (err instanceof SorobanContractError) {
    const code = err.code;
    const label = ESCROW_ERROR_LABELS[code];
    const category = classifyError(code);

    console.error(`[${category}] Error ${code}: ${label}`);

    if (code === EscrowErrorCode.LegalHoldBlocksFunding) {
      console.error("Funding is blocked by legal hold; contact admin.");
    }
  }
}
```

## Documentation

| Resource | Description |
| --- | --- |
| [Error Handling Guide](docs/error-handling.md) | How to detect, classify, and handle contract errors in your SDK integration |
| [Type Definitions](src/types.ts) | All contract types, enums, and constants |
| [Event Parser](src/events.ts) | `parseEvent` and all typed event structs |
| [Client Implementation](src/client.ts) | EscrowClient class with all entrypoints |
| [Example Usage](examples/basic-usage.ts) | Complete workflow demonstration |
| [Full Lifecycle Example](examples/full-lifecycle.ts) | End-to-end script covering init, funding, settlement, and investor claims |
| [Contract Spec](spec.json) | Machine-readable ABI (auto-generated from Rust) |
| [Event Schema Reference](../docs/EVENT_SCHEMA.md) | Authoritative topic/data layout for all 24 contract events |

## Contract Entrypoints

### State-Mutating (require authentication)

| Method | Description | Auth |
| --- | --- | --- |
| `init()` | Initialize escrow | admin |
| `fund()` | Record investor principal | investor |
| `fundWithCommitment()` | First deposit with tiered yield | investor |
| `fundBatch()` | Batch fund multiple investors | per-investor |
| `settle()` | Mark escrow as settled | sme_address |
| `withdraw()` | SME withdraws liquidity | sme_address |
| `claimInvestorPayout()` | Investor claims payout after settlement | investor |
| `cancelFunding()` | Cancel funding round | admin |
| `refund()` | Refund principal after cancellation | investor |
| `sweepTerminalDust()` | Treasury sweeps dust from terminal escrow | treasury |
| `setLegalHold()` | Admin sets/clears compliance hold | admin |
| `updateMaturity()` | Admin updates maturity timestamp | admin |
| `updateFundingTarget()` | Admin updates funding target | admin |
| `proposeAdmin()` | Admin nominates successor | admin |
| `acceptAdmin()` | Pending admin assumes role | pending admin |
| `setInvestorsAllowlisted()` | Admin manages investor allowlist | admin |
| `setAllowlistActive()` | Admin toggles allowlist gate | admin |

### Read-Only (no auth required)

| Method | Returns |
| --- | --- |
| `getEscrow()` | Full escrow state |
| `getEscrowSummary()` | Composite summary with computed fields |
| `getVersion()` | Schema version |
| `getInterfaceVersion()` | SDK interface version |
| `getFundingToken()` | Token contract ID |
| `getTreasury()` | Treasury address |
| `getContribution()` | Investor contribution amount |
| `getInvestorYieldBps()` | Investor's tier yield (bps) |
| `getInvestorClaimNotBefore()` | Investor's lock expiration (unix timestamp) |
| `isInvestorClaimed()` | Whether investor has already claimed |
| `getLegalHold()` | Whether legal hold is active |
| `getMinContributionFloor()` | Minimum contribution floor (if configured) |
| `getMaxUniqueInvestorsCap()` | Max unique investors cap (if configured) |
| `getMaxPerInvestorCap()` | Max per-investor cap (if configured) |

## Type-Safe Constants

```typescript
import {
  EscrowStatus,
  EscrowErrorCode,
  ESCROW_STATUS_LABELS,
  ESCROW_ERROR_LABELS,
  ESCROW_ERROR_CATEGORIES,
  classifyError,
  toBaseUnits,
  fromBaseUnits,
} from "@karis-ky/escrow-sdk";

// Escrow status constants
console.log(EscrowStatus.Open); // 0
console.log(EscrowStatus.Funded); // 1
console.log(EscrowStatus.Settled); // 2
console.log(ESCROW_STATUS_LABELS[EscrowStatus.Open]); // "Open"

// Error classification
const code = 103;
console.log(ESCROW_ERROR_LABELS[code]); // "Escrow not open for funding"
console.log(classifyError(code)); // "funding"
console.log(ESCROW_ERROR_CATEGORIES.funding); // { range: [100, 111], label: "Funding failure" }

// Unit conversion (7 decimal places standard)
const baseUnits = toBaseUnits("10000"); // "100000000000"
const humanReadable = fromBaseUnits(baseUnits); // "10000"
```

## Environment Setup

### Testnet

```typescript
import { SorobanServer, Networks } from "@stellar/stellar-sdk";

const rpc = new SorobanServer("https://soroban-testnet.stellar.org");

const client = new EscrowClient(
  {
    rpcUrl: rpc.serverURL.toString(),
    networkPassphrase: Networks.TESTNET_NETWORK_PASSPHRASE,
    contractId: "CCONTRACTID...",
  },
  rpc,
);
```

### Mainnet

```typescript
import { SorobanServer, Networks } from "@stellar/stellar-sdk";

const rpc = new SorobanServer("https://soroban-mainnet.stellar.org");

const client = new EscrowClient(
  {
    rpcUrl: rpc.serverURL.toString(),
    networkPassphrase: Networks.PUBLIC_NETWORK_PASSPHRASE,
    contractId: "CCONTRACTID...",
  },
  rpc,
);
```

## Full Lifecycle Example

The `examples/full-lifecycle.ts` script walks through the entire escrow lifecycle end to end: initialization, investor funding, settlement, and investor payout claims. It is intended as a runnable reference for integrating the SDK against a live Soroban network.

### Required Environment Variables

| Variable | Description |
| --- | --- |
| `SOROBAN_RPC_URL` | Soroban RPC endpoint (e.g. `https://soroban-testnet.stellar.org`) |
| `NETWORK_PASSPHRASE` | Stellar network passphrase (e.g. `Test SDF Network ; September 2015`) |
| `ESCROW_CONTRACT_ID` | Deployed LiquifactEscrow contract ID |
| `ADMIN_SECRET` | Secret key for the escrow admin account |
| `SME_SECRET` | Secret key for the SME account |
| `INVESTOR_SECRET` | Secret key for the investor account |

### Optional Environment Variables

| Variable | Description |
| --- | --- |
| `FUNDING_TOKEN` | Funding token contract ID (defaults to the native asset) |
| `TREASURY` | Treasury address for dust sweeps |
| `INVOICE_ID` | Invoice identifier used at initialization |
| `AMOUNT` | Investor contribution amount in base units |
| `YIELD_BPS` | Investor yield in basis points |
| `MATURITY` | Maturity unix timestamp |

### Running

```bash
npm run full-lifecycle
```

> **Warning:** Secret keys (`ADMIN_SECRET`, `SME_SECRET`, `INVESTOR_SECRET`) must **never** be committed to version control or written to logs. Load them from a secure secret manager or an untracked `.env` file, and use dedicated testnet-only keys when running the example.

## Schema Versioning

The SDK is versioned in lockstep with the Soroban contract schema. Each release targets a specific `SCHEMA_VERSION`:

- **SDK version 1.x** → Contract schema v6 (`SCHEMA_VERSION = 6`)
- **SDK version 2.x** (planned) → Contract schema v7 (`SCHEMA_VERSION = 7`)

Before deploying, verify your contract schema matches the SDK target:

```typescript
const schemaVersion = await client.getVersion();
const sdkTarget = SCHEMA_VERSION;

if (schemaVersion !== sdkTarget) {
  throw new Error(
    `Schema mismatch: contract v${schemaVersion}, SDK targets v${sdkTarget}`,
  );
}
```

## Building from Source

```bash
npm run build
npm run example
npm run test
```

## License

MIT
