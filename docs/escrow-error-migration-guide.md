# Error Code Migration Guide

This guide documents the migration errors (codes 90–92) that arise when upgrading or initializing a karis-ky escrow contract, how to detect them, and how to recover.

---

## Overview

The escrow contract has a `migrate()` entrypoint that gates schema version upgrades. Unlike some systems, **there is no silent migration**—all paths return typed errors when conditions aren't met. This guide explains each error code and the recovery action.

### Key Point

The `migrate()` entrypoint in schema version 6 does **not** perform any bookkeeping or data rewrites. It **only** validates version alignment. If you need to migrate data (e.g., after a struct layout change), you must:

1. Extend the `migrate()` implementation in the Rust code
2. Test the migration thoroughly
3. Deploy the new WASM
4. Call `migrate(from_version)` with the stored version

---

## Migration Error Codes

### Error 90: `MigrationVersionMismatch`

**When it occurs:**
```
You call migrate(from_version = X)
But the stored DataKey::Version is Y
And X != Y
```

**Example:**

```typescript
// Stored version in ledger: 4
// You call: migrate(5)
// Result: Error 90 — stored version is 4, not 5
```

**Recovery actions:**

1. **Check the actual stored version:**

   ```typescript
   // TypeScript SDK
   const storedVersion = await client.getVersion();
   console.log(`Stored version: ${storedVersion}`);
   ```

2. **Call migrate with the correct version:**

   ```typescript
   // Correct call
   const newVersion = await client.migrate(storedVersion);
   ```

3. **Verify after migrating:**

   ```typescript
   const updatedVersion = await client.getVersion();
   console.log(`After migration: ${updatedVersion}`);
   ```

**Rust SDK example:**

```rust
use crate::{EscrowClient, LiquifactEscrow};

let client = EscrowClient::new(env, contract_id);

// First, check stored version
let stored_version = client.get_version();
println!("Stored version: {}", stored_version);

// Call migrate with the correct from_version
let result = client.try_migrate(stored_version);
match result {
    Ok(new_version) => println!("Migrated to {}", new_version),
    Err(e) => eprintln!("Migration failed: {:?}", e),
}
```

---

### Error 91: `AlreadyCurrentSchemaVersion`

**When it occurs:**
```
You call migrate(from_version = X)
And X >= SCHEMA_VERSION of the deployed WASM
(i.e., the stored version is already at or newer than the contract)
```

**Example:**

```typescript
// WASM schema version: 6
// Stored version: 6
// You call: migrate(6)
// Result: Error 91 — already at current version, no migration needed
```

**Recovery actions:**

1. **This is not an error—it means no migration is needed.** Skip the migrate call:

   ```typescript
   const storedVersion = await client.getVersion();
   const contractVersion = 6;  // Or from contract metadata

   if (storedVersion >= contractVersion) {
       console.log("Already current; no migration needed");
       // Continue with normal operations
   } else {
       await client.migrate(storedVersion);
   }
   ```

2. **If you expected migration to happen**, check whether the schema actually changed:

   - Review the changelog in `README.md` under "Schema version changelog"
   - Confirm the WASM you deployed is a new version
   - Verify via `get_version_metadata()` for upgrade recommendations

3. **In tests**, this is expected when testing migrate logic:

   ```rust
   // Simulating an already-migrated instance
   let version_after = client.try_migrate(6);
   assert!(version_after.is_err());  // Expect error 91 (AlreadyCurrentSchemaVersion)
   ```

---

### Error 92: `NoMigrationPath`

**When it occurs:**
```
You call migrate(from_version = X)
X < SCHEMA_VERSION of the deployed WASM
And the migrate() implementation does not have a code path for X
```

**Example:**

```typescript
// Stored version: 3
// Deployed WASM schema: 6
// You call: migrate(3)
// Result: Error 92 — no migration code exists from v3 to v6
```

**Why this happens:**

In schema version 6, migration from earlier versions is not supported. The stored XDR shape changed significantly (per-investor persistent storage), and the contract does not contain the transformation logic to safely rewrite v1–v5 instances.

**Recovery actions:**

#### Option 1: Redeploy (Recommended if possible)

Create a new contract instance with the current WASM:

```bash
# Build the current WASM
cargo build --target wasm32-unknown-unknown --release -p karis-ky_escrow

# Deploy as a new contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/karis_ky_escrow.wasm \
  --source-account "$DEPLOYER_ADDRESS" \
  --secret-key "$SOURCE_SECRET" \
  --rpc-url "$SOROBAN_RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE"
```

Then:
- Initialize the new contract with `init()`
- Move active invoices to the new contract
- Update indexers and clients to point to the new contract ID

#### Option 2: Implement Migration (Advanced)

If you must keep the same contract instance (same contract ID), you can extend `migrate()` to handle older versions. This requires:

1. **Understand the old XDR:** Review the struct definitions from the old schema version
2. **Write the transform:** Implement the rewrite logic in `migrate()`
3. **Test thoroughly:** Create test cases for each transition path
4. **Deploy:** Push the new WASM with migration logic
5. **Call migrate:** Invoke the now-complete `migrate()` function

Example migration stub (in `escrow/src/lib.rs`):

```rust
pub fn migrate(env: Env, from_version: u32) -> Result<u32, EscrowError> {
    let stored_version: u32 = env.storage()
        .instance()
        .get(&DataKey::Version)
        .map(|v: u32| v)
        .unwrap_or(0);

    if stored_version != from_version {
        return Err(EscrowError::MigrationVersionMismatch);
    }

    if from_version >= SCHEMA_VERSION {
        return Err(EscrowError::AlreadyCurrentSchemaVersion);
    }

    // NEW: Implement migration paths
    match from_version {
        1 => {
            // Migrate v1 → v6
            // 1. Read old v1 escrow and per-investor data
            // 2. Rewrite to v6 persistent storage format
            // 3. Update DataKey::Version to 6
            migrate_v1_to_v6(&env)?;
            env.storage().instance().set(&DataKey::Version, &6u32);
            Ok(6)
        }
        2..=5 => {
            // Migrate v2–v5 → v6
            // Similar pattern per version
            migrate_vX_to_v6(&env, from_version)?;
            env.storage().instance().set(&DataKey::Version, &6u32);
            Ok(6)
        }
        _ => Err(EscrowError::NoMigrationPath),
    }
}

fn migrate_v1_to_v6(env: &Env) -> Result<(), EscrowError> {
    // Read old escrow at DataKey::Escrow (old struct layout)
    // Rewrite per-investor keys to persistent storage
    // Verify no data loss
    Ok(())
}
```

---

## Multi-Language Examples

### TypeScript / JavaScript

```typescript
import { EscrowClient, EscrowErrorCode, classifyError } from "@karis-ky/escrow-sdk";

async function safelyMigrate(client: EscrowClient, invoiceId: string): Promise<boolean> {
    try {
        // Step 1: Check current version
        const storedVersion = await client.getVersion();
        console.log(`Stored version: ${storedVersion}`);

        // Step 2: Get contract metadata to see if migration is needed
        const metadata = await client.getVersionMetadata();
        if (metadata.version === storedVersion) {
            console.log("Already at current schema version");
            return true;
        }

        // Step 3: Attempt migration
        console.log(`Migrating from ${storedVersion} to ${metadata.version}...`);
        const newVersion = await client.migrate(storedVersion);
        console.log(`Migration succeeded. New version: ${newVersion}`);
        return true;
    } catch (error: any) {
        const code: number = error?.code || error?.status;
        const category = classifyError(code);

        if (code === EscrowErrorCode.AlreadyCurrentSchemaVersion) {
            console.log("Already current—no action needed");
            return true;
        } else if (code === EscrowErrorCode.MigrationVersionMismatch) {
            console.error(`Version mismatch. Stored version doesn't match from_version parameter.`);
            return false;
        } else if (code === EscrowErrorCode.NoMigrationPath) {
            console.error(`No migration path available. Consider redeploying.`);
            return false;
        } else {
            console.error(`Migration error (${category}):`, error.message);
            return false;
        }
    }
}

// Usage
const client = new EscrowClient({ rpcUrl, networkPassphrase, contractId });
const success = await safelyMigrate(client, "INVOICE-2026-001");
if (!success) {
    process.exit(1);
}
```

### Python

```python
from karis_ky_escrow.client import EscrowClient
from karis_ky_escrow.types import EscrowErrorCode

def safely_migrate(client: EscrowClient, invoice_id: str) -> bool:
    """Attempt to migrate escrow to current schema version."""
    try:
        # Step 1: Check stored version
        stored_version = client.get_version()
        print(f"Stored version: {stored_version}")

        # Step 2: Get contract metadata
        metadata = client.get_version_metadata()
        if metadata['version'] == stored_version:
            print("Already at current schema version")
            return True

        # Step 3: Migrate
        print(f"Migrating from {stored_version} to {metadata['version']}...")
        new_version = client.migrate(stored_version)
        print(f"Migration succeeded. New version: {new_version}")
        return True

    except Exception as e:
        error_code = getattr(e, 'code', None)

        if error_code == EscrowErrorCode.AlreadyCurrentSchemaVersion:
            print("Already current—no action needed")
            return True
        elif error_code == EscrowErrorCode.MigrationVersionMismatch:
            print("ERROR: Version mismatch. Verify stored version.")
            return False
        elif error_code == EscrowErrorCode.NoMigrationPath:
            print("ERROR: No migration path. Consider redeploying.")
            return False
        else:
            print(f"Migration error: {e}")
            return False

# Usage
client = EscrowClient(rpc_url=rpc_url, network_passphrase=passphrase, contract_id=contract_id)
success = safely_migrate(client, "INVOICE-2026-001")
if not success:
    exit(1)
```

### Rust

```rust
use karis_ky_escrow::{LiquifactEscrow, EscrowError};
use soroban_sdk::Env;

fn safely_migrate(env: Env, contract_id: &str, from_version: u32) -> Result<u32, EscrowError> {
    let client = LiquifactEscrow::client(&env, &contract_id);

    // Step 1: Verify stored version
    let stored_version = client.get_version();
    println!("Stored version: {}", stored_version);

    if stored_version != from_version {
        eprintln!("Version mismatch: stored={}, from_version={}", stored_version, from_version);
        return Err(EscrowError::MigrationVersionMismatch);
    }

    // Step 2: Attempt migration
    match client.try_migrate(from_version) {
        Ok(new_version) => {
            println!("Migration succeeded. New version: {}", new_version);
            Ok(new_version)
        }
        Err(e) => {
            match e {
                EscrowError::AlreadyCurrentSchemaVersion => {
                    println!("Already at current schema—no migration needed");
                    Ok(stored_version)
                }
                EscrowError::NoMigrationPath => {
                    eprintln!("No migration path. Consider redeploying.");
                    Err(e)
                }
                _ => Err(e),
            }
        }
    }
}

// Usage in a test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_version_mismatch() {
        let env = Env::default();
        // Set stored version to 5
        set_stored_version(&env, 5);

        // Try to migrate with wrong from_version
        let result = safely_migrate(env, "CABC...", 4);

        assert_eq!(result, Err(EscrowError::MigrationVersionMismatch));
    }

    #[test]
    fn test_migration_already_current() {
        let env = Env::default();
        // Set stored version to 6
        set_stored_version(&env, 6);

        let result = safely_migrate(env, "CABC...", 6);

        // Should succeed with the same version or error 91
        assert!(result.is_ok() || result == Err(EscrowError::AlreadyCurrentSchemaVersion));
    }
}
```

---

## Decision Tree: When to Call `migrate()`

```
Is this a fresh deployment (new contract instance)?
│
├─ YES → Call init(), do NOT call migrate()
│
└─ NO → Does the escrow instance already exist in ledger?
         │
         ├─ NO → Call init() on new contract
         │
         └─ YES → Check the stored schema version
                  │
                  ├─ Stored version == deployed WASM schema version?
                  │   │
                  │   ├─ YES → Do NOT call migrate(); continue operations
                  │   │
                  │   └─ NO → Stored version < WASM version?
                  │           │
                  │           ├─ YES → Call migrate(stored_version)
                  │           │         If error 92 → either redeploy or extend migrate()
                  │           │
                  │           └─ NO → Stored version > WASM version (unexpected)
                  │                   You may have deployed an older WASM to a newer contract
                  │                   Review your deployment process
```

---

## Handling Migration Errors in Production

### Pre-Migration Validation

Before calling `migrate()`, validate your setup:

```bash
#!/usr/bin/env bash

# 1. Check that SOURCE_SECRET is set
if [ -z "$SOURCE_SECRET" ]; then
    echo "ERROR: SOURCE_SECRET not set"
    exit 1
fi

# 2. Verify RPC connectivity
LEDGER_INFO=$(stellar ledger-info --rpc-url "$SOROBAN_RPC_URL" 2>/dev/null)
if [ -z "$LEDGER_INFO" ]; then
    echo "ERROR: Cannot reach RPC endpoint"
    exit 1
fi

# 3. Check contract exists
CONTRACT_INFO=$(stellar contract invoke \
    --id "$CONTRACT_ID" \
    --operation get_version \
    --rpc-url "$SOROBAN_RPC_URL" 2>/dev/null)
if [ -z "$CONTRACT_INFO" ]; then
    echo "ERROR: Contract not found at $CONTRACT_ID"
    exit 1
fi

echo "Pre-migration validation passed"
```

### Retry Logic

```typescript
async function migrateWithRetry(
    client: EscrowClient,
    maxRetries: number = 3,
    delayMs: number = 2000
): Promise<number> {
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            const storedVersion = await client.getVersion();
            const newVersion = await client.migrate(storedVersion);
            return newVersion;
        } catch (error: any) {
            const code = error?.code;

            // Fatal errors—don't retry
            if (code === EscrowErrorCode.NoMigrationPath) {
                throw new Error("No migration path available (error 92). Redeploy required.");
            }
            if (code === EscrowErrorCode.AlreadyCurrentSchemaVersion) {
                return await client.getVersion();  // Already current, success
            }

            // Transient errors—retry
            if (attempt < maxRetries) {
                console.log(`Migration attempt ${attempt} failed. Retrying in ${delayMs}ms...`);
                await new Promise(resolve => setTimeout(resolve, delayMs));
            } else {
                throw error;
            }
        }
    }

    throw new Error("Migration failed after max retries");
}
```

---

## Troubleshooting Checklist

| Issue | Diagnosis | Fix |
| --- | --- | --- |
| Error 90 (VersionMismatch) | `from_version` doesn't match stored version | Call `get_version()` first; use the returned value in `migrate()` |
| Error 91 (AlreadyCurrentSchemaVersion) | Contract is already current | This is not an error; skip migrate and proceed with operations |
| Error 92 (NoMigrationPath) | No code path exists for stored → deployed version | Redeploy to a new contract instance, OR extend `migrate()` in Rust and redeploy |
| RPC call fails | Network connectivity issue or contract ID mismatch | Verify RPC URL, network passphrase, and contract ID |
| Ledger time skew | Timestamp-related validation failures during migration | Migration doesn't use time; check other entrypoint gates |
| Admin authorization fails | Not the contract admin | Verify `SOURCE_SECRET` is the admin's key |

---

## Summary

- **Error 90 (`MigrationVersionMismatch`):** You passed the wrong `from_version`. Call `get_version()` first, then pass that value.
- **Error 91 (`AlreadyCurrentSchemaVersion`):** The contract is already at current version. No migration needed. This is a success condition, not an error.
- **Error 92 (`NoMigrationPath`):** No migration code exists. Either redeploy to a new contract, or extend the `migrate()` implementation in Rust.

Always validate versions before attempting migration, and have a redeploy plan if migration is not available.
