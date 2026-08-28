# Demo 02 — Initialize an Escrow

> **Contract version:** `SCHEMA_VERSION = 6`  
> **Estimated recording length:** < 5 minutes  
> **Requires:** Demo 01 completed; `CONTRACT_ID`, `TOKEN_ID`, `ADMIN`, `SME`, `TREASURY`
> exported in your shell session

This demo calls `init` on the deployed contract and then reads back the stored
state to verify every field. By the end the escrow is in status `0` (open) and
ready to accept investor funding.

---

## Recording script

---

### Part 1 — Recap of the starting state (0:00 – 0:30)

_"We're picking up where Demo 01 left off. The contract is deployed but
uninitialised. Let me confirm the variables are set."_

```bash
echo "CONTRACT_ID: $CONTRACT_ID"
echo "TOKEN_ID:    $TOKEN_ID"
echo "ADMIN:       $ADMIN"
echo "SME:         $SME"
echo "TREASURY:    $TREASURY"
```

_"Good. These five variables are all we need to call `init`."_

---

### Part 2 — Call `init` (0:30 – 2:00)

_"The `init` entrypoint creates the invoice escrow. It's one-shot — calling it
a second time will panic. Let's walk through the key arguments before
running the command."_

_"We're setting:
- `invoice_id` to `INV001` — an ASCII alphanumeric slug, max 32 chars
- `amount` to `10000_0000000` — that's 10,000 tokens in 7-decimal base units
- `yield_bps` to `800` — 8% annualised base yield in basis points
- `maturity` to `0` — no time-lock on settlement for this demo
- `registry`, `yield_tiers`, `min_contribution`, `max_unique_investors` all
  null — we're using the minimal configuration"_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source admin \
  --network local \
  -- init \
  --admin "$ADMIN" \
  --invoice_id "INV001" \
  --sme_address "$SME" \
  --amount 10000_0000000 \
  --yield_bps 800 \
  --maturity 0 \
  --funding_token "$TOKEN_ID" \
  --registry null \
  --treasury "$TREASURY" \
  --yield_tiers null \
  --min_contribution null \
  --max_unique_investors null
```

**Expected output:**
```json
{
  "invoice_id": "INV001",
  "admin": "GADMINXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "sme_address": "GSMEXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "funding_target": "10000_0000000",
  "funded_amount": "0",
  "yield_bps": "800",
  "maturity": "0",
  "status": "0"
}
```

_"The contract returned the initial escrow state. `status: 0` means open —
no funds received yet. `funded_amount: 0` confirms that."_

---

### Part 3 — Read back the full escrow state (2:00 – 2:45)

_"We can read the full escrow state at any time using `get_escrow`. This is a
read-only call — no source key needed."_

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
  "funding_target": "10000_0000000",
  "funded_amount": "0",
  "yield_bps": "800",
  "maturity": "0",
  "status": "0"
}
```

_"Every field we passed to `init` is reflected here. `funded_amount` starts
at zero and will grow as investors contribute."_

---

### Part 4 — Read the stored schema version (2:45 – 3:15)

_"`init` writes `SCHEMA_VERSION` to on-chain storage under `DataKey::Version`.
Let's confirm that the stored version matches what we expect."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_version
```

**Expected output:**
```
"6"
```

_"Version `6` — exactly what's in the source. This version tag is the upgrade
anchor: if you ever need to redeploy or migrate, the operator runbook uses
this value to classify the transition."_

---

### Part 5 — Read optional fields (3:15 – 3:50)

_"Several optional fields return null before anything sets them. Let's check
a few."_

```bash
# No registry was provided at init — returns null
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_registry_ref

# No funding close snapshot yet (escrow not funded) — returns null
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_funding_close_snapshot

# Legal hold is not active
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_legal_hold
```

**Expected outputs:**
```
null
null
false
```

_"All as expected. The escrow is clean and open."_

---

### Part 6 — Attempt a duplicate `init` (3:50 – 4:15)

_"Let's demonstrate the one-shot property. Calling `init` again on the same
contract should panic."_

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source admin \
  --network local \
  -- init \
  --admin "$ADMIN" \
  --invoice_id "INV999" \
  --sme_address "$SME" \
  --amount 1_0000000 \
  --yield_bps 0 \
  --maturity 0 \
  --funding_token "$TOKEN_ID" \
  --registry null \
  --treasury "$TREASURY" \
  --yield_tiers null \
  --min_contribution null \
  --max_unique_investors null 2>&1 | tail -4
```

**Expected output (error):**
```
error: transaction simulation failed: HostError: Error(Contract, #1)

Caused by:
    contract call failed: "Escrow already initialized"
```

_"The contract rejected the second call. Error code `#1` corresponds to
`EscrowError::AlreadyInitialized`. This is safe by design — `init` is
idempotent in the correct direction."_

---

## Transcript summary

| Step | What happened |
|------|---------------|
| `init` | Created invoice `INV001` with a 10,000-token target at 8% yield, status → `0` (open) |
| `get_escrow` | Confirmed all fields were stored correctly |
| `get_version` | Returned `"6"` — schema version matches `SCHEMA_VERSION` constant |
| Optional reads | `get_registry_ref`, `get_funding_close_snapshot`, `get_legal_hold` all returned expected defaults |
| Duplicate `init` | Panicked with `EscrowError::AlreadyInitialized` — one-shot guarantee demonstrated |

---

## Key argument notes

| Argument | This demo | Notes |
|----------|-----------|-------|
| `amount` | `10000_0000000` | Stellar base units use 7 decimals; `10000_0000000` = 10,000 tokens |
| `yield_bps` | `800` | 8% per annum; range 0–10,000 |
| `maturity` | `0` | No time-lock; `settle` can be called immediately once funded |
| `yield_tiers` | `null` | Tiered yield disabled; all investors receive base `yield_bps` |
| `max_unique_investors` | `null` | Defaults to 128 in contract logic |

---

## Troubleshooting

**`init` returns `HostError: Error(Contract, #2)`**  
The `invoice_id` failed validation. Allowed: ASCII alphanumeric + `_`, max 32
characters. Check for spaces, special characters, or an id longer than 32 chars.

**`init` returns `HostError: Error(Contract, #7)` (InvalidFundingToken)**  
The `TOKEN_ID` address is wrong or the token contract is not deployed. Confirm
`echo $TOKEN_ID` prints a `C...` address and re-run Demo 01 Part 4.

**`get_version` returns an error instead of `"6"`**  
`init` did not succeed. Re-run Part 2 and check for errors in the output.

---

## Next

Continue to [Demo 03 — Fund as Investor](03-fund-as-investor.md).
