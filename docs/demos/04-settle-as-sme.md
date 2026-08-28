# Demo 04 — Settle as SME

> **Contract version:** `SCHEMA_VERSION = 6`  
> **Estimated recording length:** < 5 minutes  
> **Requires:** Demo 03 completed; all variables exported; escrow in status `1`
> (funded)

This demo takes the funded escrow through to settlement. The SME calls
`settle`, which moves the status from `1` → `2`. Both investors then record
their payout claims via `claim_investor_payout`. We finish by reading the
terminal state and checking the on-chain event log.

---

## Recording script

---

### Part 1 — Confirm funded state (0:00 – 0:25)

_"Let's make sure we're starting from a funded escrow."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_escrow
```

**Expected output (abbreviated):**
```json
{
  "invoice_id": "INV001",
  "funded_amount": "10000_0000000",
  "funding_target": "10000_0000000",
  "status": "1"
}
```

_"Status `1` — funded. The SME can now call `settle`."_

---

### Part 2 — Maturity gate (0:25 – 0:55)

_"Our escrow was initialised with `maturity 0`, which means there's no
time-lock on settlement. In a production invoice you'd set `maturity` to a
future Unix timestamp — the validator's ledger time must reach that value
before `settle` is allowed."_

_"To illustrate: if we had set a maturity in the future, calling `settle`
early would return this error:"_

```
error: contract call failed: "Escrow has not yet reached maturity"
```

_"Because `maturity` is `0` here, we skip straight to the call."_

---

### Part 3 — SME calls `settle` (0:55 – 1:50)

_"Only the `sme_address` configured at `init` can call `settle`. The
`--source sme` flag tells the CLI which keypair to use for signing."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source sme \
  --network local \
  -- settle
```

**Expected output:**
```json
{
  "invoice_id": "INV001",
  "funded_amount": "10000_0000000",
  "yield_bps": "800",
  "maturity": "0",
  "status": "2"
}
```

_"`status: 2` — the escrow is settled. This is a forward-only, one-way
transition. The contract emitted an `EscrowSettled` event at this point."_

---

### Part 4 — Verify settled state (1:50 – 2:15)

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_escrow
```

**Expected output:**
```json
{
  "invoice_id": "INV001",
  "admin": "GADMINXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "sme_address": "GSMEXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "funded_amount": "10000_0000000",
  "funding_target": "10000_0000000",
  "yield_bps": "800",
  "maturity": "0",
  "status": "2"
}
```

---

### Part 5 — Investor 1 claims payout (2:15 – 3:00)

_"After settlement, each investor records a payout claim marker with
`claim_investor_payout`. This is an accounting record — it marks the investor
as having claimed. Actual token disbursement happens in the integration layer
outside the contract."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source investor1 \
  --network local \
  -- claim_investor_payout \
  --investor "$INVESTOR1"
```

**Expected output:**
```
null
```

_"The `null` return is normal — the function has no return value. It wrote the
claim marker and emitted an `InvestorPayoutClaimed` event."_

---

### Part 6 — Investor 2 claims payout (3:00 – 3:20)

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source investor2 \
  --network local \
  -- claim_investor_payout \
  --investor "$INVESTOR2"
```

**Expected output:**
```
null
```

---

### Part 7 — Read claim markers (3:20 – 3:45)

_"We can verify both investors have claimed."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- is_investor_claimed \
  --investor "$INVESTOR1"
```

**Expected output:**
```
true
```

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- is_investor_claimed \
  --investor "$INVESTOR2"
```

**Expected output:**
```
true
```

---

### Part 8 — View emitted events (3:45 – 4:15)

_"The contract emits structured events at every major transition. Let's look
at what was emitted across this full lifecycle."_

```bash
stellar events \
  --id "$CONTRACT_ID" \
  --network local
```

**Expected output (abbreviated — event types):**
```
EscrowInitialized   INV001  funded_amount=0         status=0
FundReceived        INV001  investor=GINV1...        amount=5000_0000000
FundReceived        INV001  investor=GINV2...        amount=5000_0000000  status=1
EscrowSettled       INV001  funded_amount=10000...   status=2
InvestorPayoutClaimed INV001  investor=GINV1...
InvestorPayoutClaimed INV001  investor=GINV2...
```

_"Five categories of events across the four demos. Indexers and the karis-ky
backend subscribe to these to update their off-chain state."_

---

### Part 9 — Demonstrate idempotency guard (4:15 – 4:30)

_"Calling `settle` a second time is rejected because the escrow is already
in a terminal state."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source sme \
  --network local \
  -- settle 2>&1 | tail -4
```

**Expected output:**
```
error: transaction simulation failed: HostError: Error(Contract, #21)

Caused by:
    contract call failed: "Escrow is not in funded state"
```

_"Error code `#21` is `EscrowError::EscrowNotFunded`. Once settled, the status
is terminal."_

---

## Transcript summary

| Step | What happened |
|------|---------------|
| Pre-check | Confirmed status `1` (funded) before calling `settle` |
| `settle` | SME signed the transaction; status transitioned `1` → `2` (settled) |
| `get_escrow` | Confirmed all fields reflect the settled state |
| `claim_investor_payout` | Both investors recorded payout claim markers |
| `is_investor_claimed` | Returned `true` for both investors |
| `stellar events` | Showed full lifecycle event sequence |
| Duplicate `settle` | Panicked with `EscrowError::EscrowNotFunded` (#21) |

---

## What `settle` does

1. Loads the escrow and checks status is exactly `1` (funded).
2. Checks legal hold is not active.
3. Checks `maturity`: if `> 0`, asserts `ledger.timestamp() >= maturity`.
4. Calls `sme_address.require_auth()`.
5. Sets status to `2` (settled).
6. Emits `EscrowSettled` event.

> `settle` does **not** transfer tokens. The integration layer (off-chain
> backend / treasury contract) is responsible for moving principal + yield from
> the locked token pool to the SME and distributing claims to investors. The
> contract records state only.

---

## What `claim_investor_payout` does

1. Loads the escrow and checks status is exactly `2` (settled).
2. Checks legal hold is not active.
3. Checks `InvestorClaimNotBefore`: if set and `ledger.timestamp() < notBefore`,
   the call is rejected.
4. Calls `investor.require_auth()`.
5. Checks the investor is not already claimed (prevents duplicate markers).
6. Writes the claim marker to persistent storage.
7. Emits `InvestorPayoutClaimed` event.

---

## Alternative terminal path: `withdraw`

_Not shown in this demo._ If the SME wants to pull liquidity without formally
settling the invoice (status `1` → `3`), they call `withdraw` instead of
`settle`. After a `withdraw`, only `sweep_terminal_dust` is available — there
is no `claim_investor_payout` path from status `3`. The two paths are mutually
exclusive.

---

## Troubleshooting

**`settle` returns `Error(Contract, #5)` (LegalHoldActive)**  
The admin set a legal hold. In local simulation:
```bash
stellar contract invoke --id "$CONTRACT_ID" --source admin --network local \
  -- set_legal_hold --active false
```

**`settle` returns `"Escrow has not yet reached maturity"`**  
The `maturity` timestamp has not passed. Either wait or, on a local validator,
use `stellar ledger bump` (check your CLI version) to advance ledger time.
On a standard local node you can also submit dummy transactions to advance
ledger sequence and time.

**`claim_investor_payout` returns `Error(Contract, #22)` (InvestorAlreadyClaimed)**  
The investor already claimed. Each investor can only claim once. Check
`is_investor_claimed` first.

**`claim_investor_payout` returns `Error(Contract, #23)` (ClaimNotBeforeViolation)**  
The investor used `fund_with_commitment` with a commitment lock and the lock
period hasn't expired yet. Check `get_investor_claim_not_before`.

---

## Lifecycle complete

The four demos together trace a full escrow lifecycle:

```
[Deploy]         → contract deployed, no state
[Init]    (Demo 02)  → status 0: open
[Fund x2] (Demo 03)  → status 1: funded
[Settle]  (Demo 04)  → status 2: settled
[Claims]  (Demo 04)  → both investors claimed
```

For the next steps:

- **Tiered yield:** run `fund_with_commitment` in Demo 03 with a `YieldTierTable`
  configured at `init`. See [escrow-sim-stellar-cli.md](../escrow-sim-stellar-cli.md)
  section 9.
- **Dust sweep:** after settlement, a treasury admin can call
  `sweep_terminal_dust` for any rounding residue left in the contract's token
  balance.
- **Operator flows:** redeployment, migration gates, and legal hold coordination
  are covered in [OPERATOR_RUNBOOK.md](../OPERATOR_RUNBOOK.md).
- **Event schema:** full event payload reference in [EVENT_SCHEMA.md](../EVENT_SCHEMA.md).
