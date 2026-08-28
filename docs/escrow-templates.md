# Escrow Templates — `init_from_template`

> **Issue:** #251 — Add escrow template/factory support

Escrow templates let operators initialize an invoice escrow with a named configuration
profile instead of specifying every parameter by hand. This reduces error surface for
common deployment patterns and makes escrow creation consistent across scripts, UIs,
and protocol integrations.

---

## Quick start

```bash
# Initialize a 3-day fast-settlement escrow
stellar contract invoke --id $CONTRACT_ID -- \
  init_from_template \
    --template_name fast \
    --invoice_id INV_2026_001 \
    --admin $ADMIN_ADDR \
    --sme_address $SME_ADDR \
    --amount 1000000000 \
    --funding_token $TOKEN_ADDR \
    --registry null \
    --treasury $TREASURY_ADDR
```

---

## Built-in templates

Three templates are available without any prior setup:

| Name | `yield_bps` | Maturity | Profile |
|------|------------|---------|---------|
| `fast` | 200 (2 %) | 3 days (259 200 s) | Short-duration, low-yield, fastest settlement path |
| `standard` | 500 (5 %) | 14 days (1 209 600 s) | Mid-duration, moderate-yield — recommended default |
| `conservative` | 300 (3 %) | 30 days (2 592 000 s) | Long-duration, lower-yield — for high-diligence invoices |

Built-in template names are **reserved** — operators cannot register custom templates
under these names (an attempt panics with `"cannot override built-in template"`).

---

## Template parameter reference

Every template is an [`EscrowTemplate`] struct with the following fields:

| Field | Type | Description | Default when `None` |
|-------|------|-------------|---------------------|
| `yield_bps` | `i64` | Annualized yield in basis points (0–10 000). Passed directly to `init`. | Required field — no default |
| `maturity_secs` | `u64` | Duration in seconds added to the **current ledger timestamp** at call time. `0` means no maturity lock. | Required field — no default |
| `min_contribution` | `Option<i128>` | Minimum deposit per `fund`/`fund_with_commitment` call (base token units). | No minimum floor |
| `max_unique_investors` | `Option<u32>` | Cap on distinct investor addresses. | Unlimited |
| `max_per_investor` | `Option<i128>` | Per-address principal cap (base token units). | Unlimited |
| `legal_hold_clear_delay` | `Option<u64>` | Minimum seconds between `request_clear_legal_hold` and actual clearing. | Immediate clearing (0 s) |
| `funding_deadline_secs` | `Option<u64>` | Duration in seconds added to the current ledger timestamp for the funding deadline. `None` means no deadline. | No deadline |
| `yield_tiers` | `Option<Vec<YieldTier>>` | Optional tiered-yield ladder for `fund_with_commitment`. | No tiers |

### Maturity computation

The absolute `InvoiceEscrow::maturity` is computed as:

```text
maturity = ledger.timestamp() + template.maturity_secs
```

When `maturity_secs == 0`, the result is `maturity == 0`, which means **no maturity
lock** — the SME may settle immediately once the escrow is funded.

### Funding deadline computation

When `funding_deadline_secs` is set on the template, the absolute deadline stored in
`DataKey::FundingDeadline` is:

```text
funding_deadline = ledger.timestamp() + funding_deadline_secs
```

After this timestamp, new deposits are rejected with `FundingDeadlinePassed` (error 164).

---

## `init_from_template` signature

```rust
pub fn init_from_template(
    env: Env,
    template_name: String,   // "fast" | "standard" | "conservative" | custom name
    invoice_id: String,       // invoice identifier (ASCII alphanumeric + _, max 32 chars)
    admin: Address,           // initial admin (multisig recommended for production)
    sme_address: Address,     // SME / beneficiary
    amount: i128,             // funding target in base token units (must be > 0)
    funding_token: Address,   // SEP-41 token contract
    registry: Option<Address>,// optional off-chain registry hint
    treasury: Address,        // protocol treasury for dust sweeps
) -> InvoiceEscrow
```

All validation from [`init`](../escrow/src/lib.rs) applies: `invoice_id` charset/length,
`amount > 0`, `yield_bps` range, tier ordering, etc.

---

## Custom templates

Operators can register arbitrary named templates on a live contract instance. Custom
templates are stored in instance storage under `DataKey::CustomTemplate(Symbol)` and
are consulted when the `template_name` does not match a built-in.

### Register a custom template

```bash
stellar contract invoke --id $CONTRACT_ID -- \
  register_template \
    --name weekly_high \
    --template '{"yield_bps":750,"maturity_secs":604800,"min_contribution":500,...}'
```

Requires **admin** authorization. The name must be a valid Soroban `Symbol` string
(ASCII alphanumeric + `_`, max 32 chars).

### Read back a template

```bash
stellar contract invoke --id $CONTRACT_ID -- \
  get_template --name weekly_high
```

Returns the stored [`EscrowTemplate`] or `None` when the name is unknown.

---

## Example: custom template with tiered yield

```rust
// Register a 7-day template with two yield tiers
let tiers = vec![
    YieldTier { min_lock_secs: 86_400, yield_bps: 400 },   // 1-day lock → 4%
    YieldTier { min_lock_secs: 604_800, yield_bps: 900 },  // 7-day lock → 9%
];
client.register_template(
    &String::from_str(&env, "structured_7d"),
    &EscrowTemplate {
        yield_bps: 300,              // base yield (no lock)
        maturity_secs: 604_800,      // 7 days
        min_contribution: Some(100_000),
        max_unique_investors: Some(200),
        max_per_investor: None,
        legal_hold_clear_delay: Some(86_400), // 24 h two-phase hold clear
        funding_deadline_secs: Some(172_800), // 2-day funding window
        yield_tiers: Some(Vec::from_slice(&env, &tiers)),
    },
);

client.init_from_template(
    &String::from_str(&env, "structured_7d"),
    &String::from_str(&env, "INV_2026_Q3"),
    &admin,
    &sme,
    &10_000_000i128,
    &token,
    &None,
    &treasury,
);
```

---

## Security notes

- **Built-in names are write-protected.** `fast`, `standard`, and `conservative` can
  never be replaced by `register_template`. This prevents a compromised admin from
  silently changing the defaults that operators rely on.
- **Custom templates are admin-gated.** Only the current `InvoiceEscrow::admin` can
  write to `DataKey::CustomTemplate`. Production deployments should use a multisig
  admin so no single key can introduce malicious templates.
- **Template parameters pass through `init` validation.** All checks in `init`
  (yield bounds, tier ordering, min contribution vs. amount, etc.) apply regardless
  of whether the escrow was started from a template.
- **Maturity is relative, not absolute.** Two escrows initialized from the same
  template at different ledger timestamps will have different absolute maturities.
  Off-chain tooling should read `InvoiceEscrow::maturity` from the chain rather than
  deriving it from the template.

---

## Entrypoint summary

| Entrypoint | Auth | Description |
|------------|------|-------------|
| `init_from_template` | admin | Initialize escrow from a named template |
| `register_template` | admin | Store or replace a custom named template |
| `get_template` | none | Read a template (built-in or custom) by name |
