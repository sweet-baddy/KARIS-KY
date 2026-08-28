# Merkle Tree Snapshot for Funding Close

This document describes the Merkle tree alternative for the `FundingCloseSnapshot`,
the storage/computation tradeoffs, and how off-chain verifiers can validate investor
contributions using Merkle proofs.

## Motivation

The `FundingCloseSnapshot` struct is small and constant-sized (44 bytes + XDR overhead).
It stores only aggregate data: `total_principal`, `funding_target`, and ledger metadata.
**It does not store individual investor addresses or amounts.**

However, when an escrow has 1,000 or more investors, the total on-chain state is
substantial because each investor has independent **persistent** storage entries:

| Per-investor key | Size (approx) |
|-----------------|---------------|
| `InvestorContribution(Address)` | ~48 bytes |
| `InvestorEffectiveYield(Address)` | ~44 bytes |
| `InvestorClaimNotBefore(Address)` | ~44 bytes |
| `InvestorClaimed(Address)` | ~44 bytes |

At 1,000 investors, this totals approximately **180 KB** of persistent storage.

## Merkle Root Design

Instead of iterating all investor entries on-chain (which is expensive or impossible
in a single Soroban invocation), the contract stores a single **32-byte Merkle root**
at funding close:

- **Key**: `DataKey::FundingCloseMerkleRoot`
- **Type**: `BytesN<32>`
- **Hash function**: Keccak-256 (via `env.crypto().keccak256()`)
- **Leaf encoding**: `keccak256(address_string_bytes || contribution_be_bytes)`
- **Tree construction**: Binary Merkle tree with sorted sibling pairs for determinism

### Where the root is written

The root is set **once** at the moment the escrow transitions to `status == 1` (funded),
alongside `FundingCloseSnapshot`. It is **immutable** thereafter (like the snapshot itself).

At writing time, the root is the **empty Merkle root** (`keccak256("")`). This serves as
a placeholder anchor. The actual tree is constructed **off-chain** by an indexer that
has access to all investor contribution events. The admin can update the root later
if needed, or the off-chain verifier uses the root directly with the off-chain tree.

### Verification entrypoint

[`LiquifactEscrow::verify_investor_proof`] accepts:
- `investor: Address`
- `contribution: i128`
- `proof: Vec<BytesN<32>>` — the Merkle proof path (sibling hashes)

Returns `true` when the computed root matches the stored root.

## Benchmark Results

### Snapshot size at N investors

| Investors | Snapshot struct size | Per-investor persistent storage |
|-----------|---------------------|-------------------------------|
| 10        | 44 bytes            | ~1.8 KB                       |
| 100       | 44 bytes            | ~18 KB                        |
| 1,000     | 44 bytes            | ~180 KB                       |
| 10,000    | 44 bytes            | ~1.8 MB                       |

The snapshot itself is **constant O(1)** regardless of investor count. The Merkle
root adds a fixed **32 bytes** to instance storage.

### Merkle proof size

Proof depth = `ceil(log2(N))`. At each level, one 32-byte sibling is included.

| Investors | Proof depth | Proof size |
|-----------|------------|------------|
| 10        | 4          | 128 bytes  |
| 100       | 7          | 224 bytes  |
| 1,000     | 10         | 320 bytes  |
| 10,000    | 14         | 448 bytes  |

A proof for 1,000 investors is only **320 bytes** — well within Soroban's argument
size limits.

## Storage/Computation Tradeoff

### Without Merkle tree (current)
- **Storage**: O(N) persistent entries (one per investor)
- **Verification**: Cannot verify on-chain without iterating all investors
- **Gas cost for verification**: O(N) — too expensive for large N

### With Merkle tree
- **Storage**: O(1) additional instance storage (32-byte root)
- **Verification**: O(log N) hash operations (10 hashes for 1,000 investors)
- **Gas cost for verification**: O(log N) — feasible for any practical N
- **Off-chain work**: O(N log N) to build the tree (one-time, at funding close)

### When to use Merkle proofs

- **Use Merkle proofs** when: escrow has > 50 investors, or when you need
  on-chain verification of pro-rata shares without iterating storage.
- **Skip Merkle proofs** when: escrow has < 50 investors and verification
  is done off-chain via the existing `compute_investor_payout` view function.

## Security Considerations

### Second-preimage resistance
The leaf encoding uses `keccak256(address || contribution)` with domain separation
via fixed-width big-endian encoding of the contribution. An attacker cannot create
a valid leaf that collides with an interior node because:
1. Address strings are variable-length; contribution bytes are fixed 16 bytes
2. Interior nodes are always 64 bytes (two concatenated 32-byte hashes)
3. The sorted sibling order prevents proof malleability

### Trust model
The Merkle root is set at funding close by the contract itself. It is **immutable**
once written. An off-chain indexer reconstructs the tree from escrow events.
If the indexer's tree is incorrect, proofs will not validate against the on-chain
root — this is a fail-safe design.

The current implementation writes the **empty Merkle root** as a placeholder.
Future iterations can add an `update_funding_close_merkle_root` admin entrypoint
to replace the placeholder with the actual tree root once computed off-chain.

### Proof verification complexity
`verify_investor_proof` performs `1 + proof.len()` Keccak-256 hashes. Each hash
is a host function call that costs CPU but no additional storage I/O. For 1,000
investors (10-deep tree), this is 11 hashes — well within Soroban's budget.

## References

- [escrow-pro-rata.md](escrow-pro-rata.md) — pro-rata payout mathematics
- [escrow-snapshot.md](escrow-snapshot.md) — FundingCloseSnapshot lifecycle
- [ADR-003](adr/ADR-003-settlement-flow.md) — settlement flow and snapshot immutability
- [ADR-007](adr/ADR-007-storage-key-evolution.md) — additive key policy (used for `FundingCloseMerkleRoot`)
