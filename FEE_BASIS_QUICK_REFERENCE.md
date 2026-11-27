# Fee Basis - Quick Reference

## TL;DR

The protocol fee is applied **to yield only, never to principal**.

**Formula at settlement**:
```
gross_coupon = settlement_basis * yield_bps / 10_000
fee_amount = gross_coupon * fee_percentage / 10_000
net_coupon = gross_coupon - fee_amount
```

**Treasury receives**: `fee_amount` in a separate token transfer  
**Investors receive**: Pro-rata share of `net_coupon`  
**Principal**: Always remains intact (never touched)

---

## Example

```
Investor contributes:        1,000,000 USDC
Yield rate:                  800 bps (8.0%)
Fee percentage:              500 bps (5% of yield)

---SETTLEMENT---

Gross coupon = 1,000,000 * 800 / 10,000 = 80,000 USDC
Fee = 80,000 * 500 / 10,000 = 4,000 USDC
Net coupon = 80,000 - 4,000 = 76,000 USDC

---DISTRIBUTION---

Treasury receives:           4,000 USDC
Investor principal:          1,000,000 USDC (unchanged)
Investor yield:              76,000 USDC (share of net coupon)
```

---

## Key Points

| Aspect | Detail |
|--------|--------|
| **Basis** | Yield only (never principal) |
| **Applied at** | Settlement (`settle` entrypoint) |
| **Configured at** | Init (immutable thereafter) |
| **Recipient** | Treasury (separate token transfer) |
| **Transparency** | `ProtocolFeeCollected` event with full accounting |
| **Investor impact** | Yield reduced by fee; principal unchanged |
| **Storage** | `DataKey::FeePercentage` |
| **Query** | `get_fee_percentage()` |

---

## Finding the Code

| Task | File | Line Range |
|------|------|-----------|
| Read storage key | `escrow/src/storage.rs` | 138–142 |
| Read implementation | `escrow/src/admin.rs` | 745–759 |
| Read public API | `escrow/src/contract.rs` | 600–620 |
| See where applied | `escrow/src/settlement.rs` | ~450–500 |
| See tests | `escrow/src/tests/fee_collection.rs` | Full module |
| Full documentation | `docs/fee-basis-and-collection.md` | Complete reference |

---

## Common Questions

**Q: Can the fee be applied to principal?**  
A: No. The fee is always applied to yield, never to principal. This is a design constraint.

**Q: Can the admin change the fee after init?**  
A: No. The fee is immutable after initialization. Only a contract redeploy can change it.

**Q: Who receives the fee?**  
A: The treasury address (configured at init).

**Q: When is the fee applied?**  
A: During settlement, when the escrow transitions to status 2.

**Q: How do I verify the fee was applied correctly?**  
A: Query the `ProtocolFeeCollected` event at settlement and verify:
  ```
  expected_fee = gross_coupon * fee_percentage / 10_000
  Verify: expected_fee == event.fee_amount
  ```

**Q: What if fee_percentage is 0?**  
A: No fee is collected and no event is emitted. The full yield goes to investors.

**Q: What about partial settlements?**  
A: The fee is computed on the settled amount, not the total funded amount:
  ```
  Funded: 1,000,000
  Settled: 500,000 (partial)
  
  Gross coupon = 500,000 * 800 / 10,000 = 40,000
  Fee = 40,000 * 500 / 10_000 = 2,000
  ```

---

## For Different Audiences

### Treasury Operators
- Read: `docs/fee-basis-and-collection.md` → "Treasury Operations" section
- Verify via: `ProtocolFeeCollected` events
- Reconcile with: Sum of all event `fee_amount` values

### Developers
- Storage key: `DataKey::FeePercentage` in `escrow/src/storage.rs`
- Implementation: `get_fee_percentage_impl()` in `escrow/src/admin.rs`
- Tests: `escrow/src/tests/fee_collection.rs`

### Auditors
- Verify basis: Check `settle_impl()` only modifies yield, not principal
- Verify application: Fee is in `settle`, not in `fund` or `claim`
- Verify treasury: Look for `transfer_funding_token_with_balance_checks()` call

### Investors
- Principal: Safe (never touched)
- Yield: Reduced by configured fee percentage
- Event: `ProtocolFeeCollected` shows exact deduction

---

## Verification Checklist

- [ ] `get_fee_percentage()` returns configured value
- [ ] Fee percentage is in basis points (0–10,000)
- [ ] Fee is deducted only from yield, not principal
- [ ] Treasury receives separate transfer
- [ ] `ProtocolFeeCollected` event is emitted with fee details
- [ ] Partial settlements compute fees on settled amount only
- [ ] Zero fee (0 bps) results in no transfer or event
- [ ] Fee is applied at settlement, not funding or claim time

---

## Related Documents

- Full reference: `docs/fee-basis-and-collection.md`
- Completion summary: `FEE_BASIS_DOCUMENTATION_COMPLETE.md`
- Tests: `escrow/src/tests/fee_collection.rs`

---

**Last updated**: 2024  
**Status**: Documented and tested
