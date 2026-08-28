# Off-Chain Storage with Merkle Proof Verification

**Replaces:** Persistent per-investor storage for large escrows (10,000+ investors)  
**Status:** Design (RFC-004 DRAFT)  
**Target Release:** v2.1 (Q2 2027)

---

## Overview

For escrows with 10,000+ investors, on-chain storage becomes prohibitively expensive:
- Each investor record: ~50 bytes persistent storage
- 10,000 investors: ~500 KB
- 100,000 investors: ~5 MB (hits practical Soroban limits)
- TTL rent: scales with investor count

**Solution:** Store investor records off-chain (IPFS, S3, Arweave) with on-chain Merkle root verification.

---

## Architecture

### Storage Modes

**DRAFT Mode (current, default):**
```
Escrow Instance
├─ InvoiceEscrow (on-chain, ~300 bytes)
├─ InvestorContribution(addr) × n (persistent keys, on-chain)
│  └─ 50 bytes each × n investors
├─ InvestorEffectiveYield(addr) × n
├─ InvestorClaimNotBefore(addr) × n
└─ InvestorClaimed(addr) × n

Storage cost: O(n) per investor
TTL extensions: O(n) separate keys
Practical limit: ~1,000–2,000 investors
```

**MERKLE Mode (new, opt-in):**
```
Escrow Instance (on-chain)
├─ InvoiceEscrow (~300 bytes)
├─ InvestorStorageMode: "MERKLE"
├─ InvestorDataMerkleRoot (32 bytes)
├─ InvestorDataSource (IPFS CID or URL, ~100 bytes)
└─ MerkleRootUpdatedAt (8 bytes)

Off-Chain Ledger (IPFS/S3/Arweave)
└─ investor_ledger.jsonl (immutable)
   ├─ { investor: GXXX, contribution: 1M, yield_bps: 500, ... }
   ├─ { investor: GYYY, contribution: 500K, yield_bps: 600, ... }
   └─ ... (50K+ records)

Storage cost: O(1) on-chain, O(n) off-chain (no rent)
TTL extensions: Single root key, not O(n)
Practical limit: ~1M investors
```

---

## Data Model

### On-Chain (Instance Storage)

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// "DRAFT" (default) or "MERKLE" — determines investor data location
    /// Immutable after init
    InvestorStorageMode,
    
    /// Merkle root of investor records tree (SHA-256 hash, 32 bytes)
    /// Only present when StorageMode == MERKLE
    /// Immutable after init
    InvestorDataMerkleRoot,
    
    /// URL/CID pointing to off-chain investor ledger
    /// Example: "ipfs://QmABC123..." or "s3://bucket/ledger.jsonl"
    /// Only present when StorageMode == MERKLE
    /// Immutable after init
    InvestorDataSource,
    
    /// Ledger timestamp when merkle root was set (for audit)
    /// Optional; used to track root age
    MerkleRootUpdatedAt,
}
```

**Storage footprint comparison:**

| Mode | Root | Source | Subtotal | Per Investor | Total (100K) |
|------|------|--------|----------|--------------|--------------|
| DRAFT | — | — | ~300 B | 50 B | 5 MB |
| MERKLE | 32 B | 100 B | ~300 B | — | 300 B |

**Savings:** ~16× reduction for 100K investors

### Off-Chain (IPFS/S3/Arweave)

**Investor Record (line-delimited JSON):**

```json
{
  "investor": "GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "contribution": 1000000,
  "contribution_timestamp": 1722090119,
  "effective_yield_bps": 500,
  "claim_lock_timestamp": 1725000000,
  "claimed": false,
  "claim_timestamp": null
}
```

**Size:** ~150 bytes per record (JSON + newline)  
**Format:** JSONL (line-delimited for streaming/partial reads)

**Ledger File:**
```
ledger.jsonl
├─ Immutable (no updates after creation)
├─ Content-addressed (IPFS CID = SHA-256 of content)
├─ Sorted by investor address (for efficient lookups)
└─ Size: 150 B × n investors ≈ 15 MB for 100K investors
```

### Merkle Tree

**Structure (binary tree, balanced):**

```
                    Root: sha256(...)
                    /               \
            sha256(...) ─────── sha256(...)
            /         \        /        \
        sha256  sha256  sha256  sha256  sha256
        /    \  /   \   /   \  /   \   /
    L1  L2  L3  L4  L5  L6  L7  L8  L9
   [R1][R2][R3][R4][R5][R6][R7][R8][R9] ...
```

**Leaf hash:** `sha256(keccak256(investor_record_abi_encoded))`  
**Internal nodes:** `sha256(left_hash || right_hash)`  
**Root:** Single commitment to all records

**Proof for investor at index i:**

```
Proof = [sibling_1, sibling_2, ..., sibling_log2(n)]
Size = 32 bytes × log₂(n)
Examples:
  - 1K investors: 32 × 10 = 320 bytes
  - 10K investors: 32 × 14 = 448 bytes
  - 100K investors: 32 × 17 = 544 bytes
  - 1M investors: 32 × 20 = 640 bytes
```

**Verification algorithm (pseudocode):**

```python
def verify_merkle_proof(investor_record, proof, stored_root):
    # 1. Compute leaf hash
    leaf_hash = sha256(keccak256(abi_encode(investor_record)))
    
    # 2. Walk up tree using proof siblings
    current_hash = leaf_hash
    for i, sibling in enumerate(proof):
        # Alternate between left/right based on bit position
        if (proof_index >> i) & 1 == 0:
            current_hash = sha256(current_hash || sibling)
        else:
            current_hash = sha256(sibling || current_hash)
    
    # 3. Verify final hash matches stored root
    return current_hash == stored_root
```

---

## On-Chain Integration

### Initialization

**`init()` with merkle parameters:**

```rust
pub fn init(
    env: Env,
    // ... existing parameters ...
    use_merkle_storage: bool,           // NEW: false = DRAFT, true = MERKLE
    investor_data_source: String,       // NEW: IPFS CID or URL
    investor_data_merkle_root: BytesN<32>,  // NEW: computed off-chain
) -> Result<EscrowSummary, EscrowError> {
    // Validate inputs
    if use_merkle_storage {
        // Merkle root cannot be zero (would be too permissive)
        if investor_data_merkle_root.is_zero() {
            return Err(EscrowError::InvalidMerkleRoot);  // error code 50
        }
        
        // Store mode
        env.storage().instance().set(
            &DataKey::InvestorStorageMode,
            &"MERKLE",
        );
        
        // Store root + source immutably
        env.storage().instance().set(
            &DataKey::InvestorDataMerkleRoot,
            &investor_data_merkle_root,
        );
        
        env.storage().instance().set(
            &DataKey::InvestorDataSource,
            &investor_data_source,
        );
        
        // Timestamp for audit
        env.storage().instance().set(
            &DataKey::MerkleRootUpdatedAt,
            &env.ledger().timestamp(),
        );
    } else {
        // DRAFT mode (default, backward compatible)
        env.storage().instance().set(
            &DataKey::InvestorStorageMode,
            &"DRAFT",
        );
        // No other merkle keys set
    }
    
    // ... rest of init ...
}
```

### Claiming with Merkle Proof

**`claim_investor_payout()` with merkle verification:**

```rust
pub fn claim_investor_payout(
    env: Env,
    investor_contribution: i128,
    effective_yield_bps: i64,
    claim_lock_timestamp: u64,
    merkle_proof: Option<Vec<BytesN<32>>>,
) -> Result<InvestorPayoutClaimed, EscrowError> {
    let caller = env.current_contract_id();  // or caller address
    
    // Get storage mode
    let mode = env.storage().instance()
        .get(&DataKey::InvestorStorageMode)
        .unwrap_or("DRAFT".to_string());
    
    match mode.as_str() {
        "DRAFT" => {
            // Traditional on-chain verification
            verify_claim_draft_mode(&env, &caller, investor_contribution)?;
        }
        "MERKLE" => {
            // Merkle proof verification
            let proof = merkle_proof
                .ok_or(EscrowError::MissingMerkleProof)?;  // error 51
            
            verify_claim_merkle_mode(
                &env,
                &caller,
                investor_contribution,
                effective_yield_bps,
                claim_lock_timestamp,
                proof,
            )?;
        }
        _ => {
            return Err(EscrowError::InvalidStorageMode);  // error 52
        }
    }
    
    // ... rest of claim logic (same for both modes) ...
}
```

**Merkle verification function:**

```rust
fn verify_claim_merkle_mode(
    env: &Env,
    investor: &Address,
    contribution: i128,
    effective_yield_bps: i64,
    claim_lock_timestamp: u64,
    merkle_proof: Vec<BytesN<32>>,
) -> Result<(), EscrowError> {
    // 1. Get stored merkle root
    let stored_root = env.storage().instance()
        .get(&DataKey::InvestorDataMerkleRoot)?;
    
    // 2. Construct investor record (must match off-chain)
    let record = InvestorRecord {
        investor: investor.clone(),
        contribution,
        effective_yield_bps,
        claim_lock_timestamp,
    };
    
    // 3. Compute leaf hash
    let encoded = env.serialize(&record)?;
    let leaf_hash = sha256(keccak256(&encoded));
    
    // 4. Verify merkle path
    let computed_root = verify_merkle_path(leaf_hash, merkle_proof)?;
    
    // 5. Compare roots
    if computed_root != stored_root {
        return Err(EscrowError::InvalidMerkleProof);  // error 50
    }
    
    Ok(())
}
```

**Merkle tree verification:**

```rust
fn verify_merkle_path(
    leaf_hash: BytesN<32>,
    proof: Vec<BytesN<32>>,
) -> Result<BytesN<32>, EscrowError> {
    let mut current = leaf_hash;
    
    for sibling in proof {
        // Determine left/right ordering based on hash values
        // (simplified; real implementation uses bit flags in proof)
        if current.0 < sibling.0 {
            // current is left
            current = sha256(&[&current.0, &sibling.0].concat());
        } else {
            // current is right
            current = sha256(&[&sibling.0, &current.0].concat());
        }
    }
    
    Ok(current)
}
```

### Merkle Root Updates (Future)

**New entrypoint (v2.2+): `update_merkle_root()`**

For incremental updates (e.g., new investors join after initial merkle tree):

```rust
pub fn update_merkle_root(
    env: Env,
    new_merkle_root: BytesN<32>,
    new_data_source: String,
) -> Result<(), EscrowError> {
    let escrow = Self::get_escrow(env.clone())?;
    
    // Only admin can update merkle root
    escrow.admin.require_auth();
    
    // Check escrow is still open (status == 0)
    if escrow.status != 0 {
        return Err(EscrowError::EscrowNotOpen);
    }
    
    // Update root (old root is lost; for auditing, index off-chain)
    env.storage().instance().set(
        &DataKey::InvestorDataMerkleRoot,
        &new_merkle_root,
    );
    
    env.storage().instance().set(
        &DataKey::InvestorDataSource,
        &new_data_source,
    );
    
    env.storage().instance().set(
        &DataKey::MerkleRootUpdatedAt,
        &env.ledger().timestamp(),
    );
    
    // Emit event
    env.events().publish(("merkle_root_updated", escrow.invoice_id), (new_merkle_root,));
    
    Ok(())
}
```

**Caveat:** Allows admin to change merkle root during open phase. Integrators must trust admin + verify off-chain ledger is being correctly maintained.

---

## Off-Chain Tooling

### Merkle Tree Generation (Python)

```python
# pseudocode: merkle_tree_builder.py

import hashlib
import json
from typing import List, Dict, Tuple

class InvestorRecord:
    def __init__(self, investor: str, contribution: int, effective_yield_bps: int, 
                 claim_lock_timestamp: int):
        self.investor = investor
        self.contribution = contribution
        self.effective_yield_bps = effective_yield_bps
        self.claim_lock_timestamp = claim_lock_timestamp
    
    def to_abi_encoded(self) -> bytes:
        # Soroban ABI encoding (similar to EVM ABI)
        return (
            self.investor.encode() +
            self.contribution.to_bytes(16, 'big') +
            self.effective_yield_bps.to_bytes(8, 'big') +
            self.claim_lock_timestamp.to_bytes(8, 'big')
        )

class MerkleTree:
    def __init__(self, records: List[InvestorRecord]):
        self.records = sorted(records, key=lambda r: r.investor)  # Sort for consistency
        self.leaves = [self._leaf_hash(r) for r in self.records]
        self.tree = self._build_tree(self.leaves)
    
    @staticmethod
    def _leaf_hash(record: InvestorRecord) -> bytes:
        # Double hash: keccak256 then sha256
        encoded = record.to_abi_encoded()
        keccak_hash = hashlib.sha3_256(encoded).digest()
        return hashlib.sha256(keccak_hash).digest()
    
    @staticmethod
    def _node_hash(left: bytes, right: bytes) -> bytes:
        return hashlib.sha256(left + right).digest()
    
    def _build_tree(self, leaves: List[bytes]) -> List[List[bytes]]:
        if not leaves:
            return []
        
        tree = [leaves]
        current_level = leaves
        
        while len(current_level) > 1:
            next_level = []
            for i in range(0, len(current_level), 2):
                left = current_level[i]
                right = current_level[i + 1] if i + 1 < len(current_level) else left
                next_level.append(self._node_hash(left, right))
            tree.append(next_level)
            current_level = next_level
        
        return tree
    
    def root(self) -> bytes:
        if not self.tree:
            return b''
        return self.tree[-1][0]
    
    def proof(self, leaf_index: int) -> List[bytes]:
        """Generate merkle proof for leaf at leaf_index"""
        proof = []
        index = leaf_index
        
        for level in self.tree[:-1]:  # Exclude root
            sibling_index = index ^ 1  # XOR to get sibling
            if sibling_index < len(level):
                proof.append(level[sibling_index])
            index //= 2
        
        return proof

def generate_ledger_and_merkle(investor_data: List[Dict]) -> Tuple[str, bytes]:
    """
    Generate JSONL ledger file and compute merkle root
    
    Args:
        investor_data: List of { investor, contribution, effective_yield_bps, ... }
    
    Returns:
        (ledger_jsonl, merkle_root_hex)
    """
    records = [
        InvestorRecord(
            investor=d['investor'],
            contribution=d['contribution'],
            effective_yield_bps=d['effective_yield_bps'],
            claim_lock_timestamp=d['claim_lock_timestamp']
        )
        for d in investor_data
    ]
    
    # Generate ledger
    ledger_lines = [json.dumps(r.__dict__) for r in records]
    ledger_jsonl = '\n'.join(ledger_lines)
    
    # Compute merkle tree + root
    tree = MerkleTree(records)
    root = tree.root()
    
    return ledger_jsonl, root.hex()
```

### Integration Flow (High-Level)

**Off-chain integrator workflow:**

```
1. Collect investor contributions (over funding period)
   investor_data = [
       { investor: "GXXX...", contribution: 1000000, ... },
       { investor: "GYYY...", contribution: 500000, ... },
       ...
   ]

2. Generate ledger + merkle tree
   ledger_jsonl, merkle_root = generate_ledger_and_merkle(investor_data)

3. Upload ledger to IPFS
   ipfs_cid = ipfs_client.add_bytes(ledger_jsonl)  # Returns QmABC123...

4. Initialize escrow with merkle root
   escrow.init(
       use_merkle_storage=true,
       investor_data_source=f"ipfs://{ipfs_cid}",
       investor_data_merkle_root=bytes.fromhex(merkle_root)
   )

5. When investor claims:
   a. Fetch ledger from IPFS
   b. Find investor record in ledger
   c. Generate merkle proof for that record
   d. Submit claim with proof
      escrow.claim_investor_payout(
          investor_contribution=1000000,
          effective_yield_bps=500,
          claim_lock_timestamp=1725000000,
          merkle_proof=[proof_siblings...]
      )

6. Contract verifies proof against stored root ✓
```

---

## Performance Analysis

### Storage Savings

| Metric | DRAFT (100K inv) | MERKLE (100K inv) | Reduction |
|--------|------------------|-------------------|-----------|
| On-chain storage | 5 MB | 300 B | **16,667×** |
| Per-investor TTL | 100K separate | 1 root | **100,000×** |
| TTL extension cost | O(n) → 50 calls | O(1) → 1 call | **50×** |
| Proof size | — | 544 B | — |

### Computation

| Operation | Time | Notes |
|-----------|------|-------|
| Generate 100K ledger | ~100 ms | Off-chain |
| Compute merkle tree | ~500 ms | Off-chain |
| Verify proof on-chain | ~10 ms | 17 hash ops (log₂(100K)) |
| Upload ledger to IPFS | ~2–5 sec | Network dependent |

### Network/Latency

| Operation | Latency |
|-----------|---------|
| Fetch ledger from IPFS | 100–500 ms (first fetch, cached after) |
| Generate proof locally | < 1 ms (O(log n)) |
| Submit claim + verify on-chain | 5–10 sec (Soroban block time) |

---

## Migration Path

### No Forced Migration

- DRAFT mode escrows continue indefinitely
- MERKLE mode is opt-in at init
- Can coexist on same blockchain

### Optional Migration (Manual)

If existing DRAFT escrow wants to switch to MERKLE (future enhancement):

1. Admin exports all on-chain investor records
2. Build off-chain ledger + merkle tree
3. Call `migrate_to_merkle_storage(new_root, new_source)`
4. Future claims use merkle verification

---

## Error Codes

| Code | Error | Meaning |
|------|-------|---------|
| 50 | InvalidMerkleProof | Proof does not verify against stored root |
| 51 | MissingMerkleProof | Claim in MERKLE mode but no proof provided |
| 52 | InvalidStorageMode | StorageMode is not "DRAFT" or "MERKLE" |
| 53 | InvalidMerkleRoot | Merkle root is zero/invalid at init |
| 54 | DataSourceNotFound | Off-chain ledger unreachable |

---

## Security Model

### Threat Model

| Threat | Mitigation |
|--------|-----------|
| Attacker forges proof for non-existent investor | Root is cryptographically bound; forgery requires hash collision (2^128 cost) |
| Attacker modifies off-chain ledger | IPFS content-addresses by hash; any modification changes CID (discoverable) |
| Integrator loses off-chain ledger | IPFS replication + backup to S3/Arweave; data is public + recoverable |
| Admin updates merkle root during open phase | Allowed by design; integrators must trust admin + verify ledger |

### Assumptions

1. **Hash function security:** keccak256 + sha256 are collision-resistant
2. **Off-chain availability:** IPFS/S3/Arweave are reasonably available (99%+ uptime)
3. **Integration trust:** Admin maintains accurate off-chain ledger
4. **Clock accuracy:** Ledger timestamps are accurate (tied to Soroban ledger time)

---

## References

- RFC-004: Off-Chain Storage with Merkle Proofs
- Merkle Tree Wikipedia: https://en.wikipedia.org/wiki/Merkle_tree
- Ethereum Merkle Proofs: https://ethereum.org/en/developers/docs/data-structures-and-encoding/merkle-proofs/
- IPFS: https://ipfs.io
- Arweave: https://www.arweave.org

