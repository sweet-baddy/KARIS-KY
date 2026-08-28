# Demo 03 — Fund as Investor

> **Contract version:** `SCHEMA_VERSION = 6`  
> **Estimated recording length:** < 5 minutes  
> **Requires:** Demo 02 completed; all variables from Demo 01 exported in your
> shell session; escrow in status `0` (open)

This demo shows two investors splitting the 10,000-token funding target using
the `fund` entrypoint. The second investor's deposit triggers the status
transition from `0` (open) → `1` (funded) and captures the
`FundingCloseSnapshot`. We then read back contribution state and the snapshot.

---

## Recording script

---

### Part 1 — Recap (0:00 – 0:25)

_"We have an initialised escrow for invoice `INV001` with a 10,000-token
target, currently at status `0` open. Let's confirm before touching anything."_

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
  "funded_amount": "0",
  "funding_target": "10000_0000000",
  "status": "0"
}
```

---

### Part 2 — Investor 1 funds 5,000 tokens (0:25 – 1:20)

_"Investor 1 sends 5,000 tokens — half the target. The `--source` flag tells
the CLI which keypair signs the transaction, and `--investor` is the address
that the contract records the contribution under. They must match."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source investor1 \
  --network local \
  -- fund \
  --investor "$INVESTOR1" \
  --amount 5000_0000000
```

**Expected output:**
```json
{
  "invoice_id": "INV001",
  "funded_amount": "5000_0000000",
  "status": "0"
}
```

_"`funded_amount` is now 5,000 but `status` is still `0` — we haven't hit the
target yet."_

---

### Part 3 — Read Investor 1's contribution (1:20 – 1:45)

_"We can check how much any individual address has contributed."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_contribution \
  --investor "$INVESTOR1"
```

**Expected output:**
```
"5000_0000000"
```

```bash
# Investor 2 has not funded yet — returns 0 or null
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_contribution \
  --investor "$INVESTOR2"
```

**Expected output:**
```
"0"
```

---

### Part 4 — Investor 2 funds the remaining 5,000 (1:45 – 2:40)

_"Investor 2 covers the remaining half. This deposit hits the funding target,
so the contract transitions to status `1` (funded) and records the
`FundingCloseSnapshot`."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source investor2 \
  --network local \
  -- fund \
  --investor "$INVESTOR2" \
  --amount 5000_0000000
```

**Expected output:**
```json
{
  "invoice_id": "INV001",
  "funded_amount": "10000_0000000",
  "status": "1"
}
```

_"`status: 1` — the escrow is now fully funded. The funding target was reached
in this single call."_

---

### Part 5 — Verify the full escrow state (2:40 – 3:05)

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
  "funded_amount": "10000_0000000",
  "funding_target": "10000_0000000",
  "yield_bps": "800",
  "status": "1"
}
```

---

### Part 6 — Read the funding close snapshot (3:05 – 3:40)

_"The `FundingCloseSnapshot` is a single-write immutable record captured at
the moment the target was first reached. It freezes the pro-rata denominator
so yield calculations can't be gamed by late deposits."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_funding_close_snapshot
```

**Expected output:**
```json
{
  "total_principal": "10000_0000000",
  "funding_target": "10000_0000000",
  "closed_at_ledger_timestamp": "1753433344",
  "closed_at_ledger_sequence": "42"
}
```

_"The timestamp and ledger sequence are from the validator's perspective —
they match the ledger when investor 2's `fund` transaction was applied."_

---

### Part 7 — Investor count and contribution reads (3:40 – 4:10)

```bash
# How many distinct funders?
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_unique_funder_count
```

**Expected output:**
```
"2"
```

```bash
# Investor 2's contribution
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_contribution \
  --investor "$INVESTOR2"
```

**Expected output:**
```
"5000_0000000"
```

```bash
# Effective yield for investor 1 (no tiered commitment — base yield applies)
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_investor_yield_bps \
  --investor "$INVESTOR1"
```

**Expected output:**
```
"800"
```

_"Both investors are recorded at the 8% base yield. In Demo 05 (not shown
here) you'd use `fund_with_commitment` to lock capital for a higher-tier
yield from the `YieldTierTable`. For this demo we're keeping things simple."_

---

### Part 8 — Attempt to fund a closed escrow (4:10 – 4:30)

_"Once the escrow is funded, additional deposits are rejected."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source investor1 \
  --network local \
  -- fund \
  --investor "$INVESTOR1" \
  --amount 1_0000000 2>&1 | tail -4
```

**Expected output:**
```
error: transaction simulation failed: HostError: Error(Contract, #20)

Caused by:
    contract call failed: "Escrow is not open for funding"
```

_"Error code `#20` is `EscrowError::EscrowNotOpen`. The state machine enforces
a forward-only transition: once funded you cannot go back to open."_

---

## Transcript summary

| Step | What happened |
|------|---------------|
| Investor 1 funds | `fund` recorded 5,000 tokens; status stayed `0` (open) |
| Investor 2 funds | `fund` recorded 5,000 tokens; total hit target; status → `1` (funded) |
| `FundingCloseSnapshot` | Written at the moment the target was reached; immutable denominator for pro-rata yield |
| `get_unique_funder_count` | Returned `"2"` |
| Effective yield | Both investors at base 8% (`800` bps) |
| Over-fund attempt | Panicked with `EscrowError::EscrowNotOpen` (#20) |

---

## How `fund` works

1. Loads the escrow state and checks status is `0` (open).
2. Checks legal hold is not active.
3. If `min_contribution` was set, checks the amount meets the floor.
4. If the investor is new, checks unique investor count is under the cap and
   increments `UniqueFunderCount`.
5. Calls `require_auth()` on the `investor` address.
6. Adds `amount` to the investor's stored contribution via `checked_add`.
7. Adds `amount` to `funded_amount` via `checked_add`.
8. If `funded_amount >= funding_target`, sets status to `1` and writes the
   `FundingCloseSnapshot`.
9. Emits an event and returns the updated escrow summary.

> In Schema version 6 per-investor keys live in **persistent** storage to
> decouple their TTL from the contract instance. This is why `get_contribution`
> remains readable independent of instance storage renewal.

---

## Troubleshooting

**`fund` returns `Error(Contract, #13)` (InvestorCapExceeded)**  
The escrow has reached its `max_unique_investors` cap. Either use an already-
registered investor address or redeploy with a higher cap.

**`fund` returns `Error(Contract, #5)` (LegalHoldActive)**  
The admin activated a legal hold. Check `get_legal_hold` and, if you control
the admin key in local simulation, clear it with:
```bash
stellar contract invoke --id "$CONTRACT_ID" --source admin --network local \
  -- set_legal_hold --active false
```

**`get_contribution` returns `"0"` right after funding**  
Confirm the transaction succeeded without errors and the correct `INVESTOR1`/
`INVESTOR2` address variable is set. Re-run `echo $INVESTOR1`.

---

## Next

Continue to [Demo 04 — Settle as SME](04-settle-as-sme.md).
