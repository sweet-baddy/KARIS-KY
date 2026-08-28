# Canary Deployment: RBAC and Access Control

Role-based access control for staged deployments with CanaryOperator and Admin separation.

---

## Role Hierarchy

```
GOVERNANCE (multisig)
  │
  ├─ Approves: canary stage, production promotion
  │
  ├─► ADMIN (multisig or governed contract)
  │    │
  │    ├─ Deploys to production instances
  │    ├─ Manages production contract state
  │    └─ Escalation point for emergency
  │
  └─► CANARY_OPERATOR (individual or small group)
       │
       ├─ Deploys WASM to canary instances only
       ├─ Manages canary monitoring
       └─ Can rollback canary (not production)
```

---

## Permission Matrix

| Action | CanaryOperator | Admin | Governance |
|--------|---|---|---|
| **Deployment** | | | |
| Upload WASM | ✓ | ✓ | — |
| Deploy to canary (is_canary=true) | ✓ | — | — |
| Deploy to production (is_canary=false) | ✗ | ✓ | Approves |
| Set canary flag (init) | ✓ | ✓ | — |
| Update canary flag (admin entrypoint) | ✗ | ✓ | — |
| **Escrow Management** | | | |
| Set legal hold on canary | ✓ | — | — |
| Set legal hold on production | ✗ | ✓ | — |
| Clear legal hold on canary | ✓ | — | — |
| Clear legal hold on production | ✗ | ✓ | — |
| **Canary Operations** | | | |
| Query canary instance state | ✓ | ✓ | — |
| Query production instance state | ✓ | ✓ | — |
| Monitor canary metrics | ✓ | ✓ | — |
| Promote canary to production | ✗ | ✓ | Approves |
| **Rollback** | | | |
| Rollback canary WASM | ✓ | — | — |
| Rollback production WASM | ✗ | ✓ | Consult |
| **Access Management** | | | |
| Add CanaryOperator | ✗ | ✓ | Approves |
| Remove CanaryOperator | ✗ | ✓ | Approves |
| Rotate Admin key | ✗ | ✓ | Approves |

---

## Access Control Implementation

### Contract-level RBAC (pseudo-code)

```rust
pub enum AccessLevel {
    CanaryOperator,
    Admin,
    Governance,
}

#[derive(Clone)]
pub struct AccessControl {
    pub canary_operators: Set<Address>,
    pub admin: Address,
}

// Check if caller is CanaryOperator
fn require_canary_operator(env: &Env) -> Address {
    let caller = env.invoker();
    let ac = env.storage().instance()
        .get::<_, AccessControl>(&DataKey::AccessControl)
        .unwrap();
    
    if !ac.canary_operators.contains(&caller) {
        fail(env, EscrowError::UnauthorizedCanaryOperator);
    }
    caller
}

// Check if caller is Admin
fn require_admin(env: &Env) -> Address {
    let ac = env.storage().instance()
        .get::<_, AccessControl>(&DataKey::AccessControl)
        .unwrap();
    ac.admin.require_auth();
    ac.admin.clone()
}

// Upgrade with canary check
pub fn upgrade_wasm(env: Env, new_wasm_hash: BytesN<32>) {
    let escrow = Self::load_escrow_require_admin(env.clone());
    
    if escrow.is_canary {
        // Canary instance: CanaryOperator OR Admin
        let caller = env.invoker();
        let ac = env.storage().instance()
            .get::<_, AccessControl>(&DataKey::AccessControl)
            .unwrap();
        
        let is_canary_op = ac.canary_operators.contains(&caller);
        let is_admin = caller == ac.admin;
        
        if !is_canary_op && !is_admin {
            fail(&env, EscrowError::UnauthorizedCanaryOperator);
        }
    } else {
        // Production instance: Admin only
        let ac = env.storage().instance()
            .get::<_, AccessControl>(&DataKey::AccessControl)
            .unwrap();
        ac.admin.require_auth();
    }
    
    env.deployer().update_current_contract_wasm(new_wasm_hash);
}
```

---

## Key Management Strategy

### CanaryOperator key

**Type:** Individual or small group (not multisig, for operational efficiency)

**Storage:**
```bash
# Local, encrypted
~/.karis-ky/canary_operator_secret.enc

# Unlock before canary deployment
export CANARY_OPERATOR_SECRET=$(decrypt_secret canary_operator_secret.enc)
```

**Permissions:**
- Can deploy to canary only
- Can monitor all instances
- Cannot access production
- Cannot escalate or promote

**Rotation:** Annually or after incident

### Admin key

**Type:** Multisig (3-of-5 governance members)

**Storage:** Cold storage (offline)

**Permissions:**
- Can deploy to production
- Can escalate and rollback
- Requires threshold signatures for any action

**Rotation:** Per governance policy (typically 6-12 months)

---

## Deployment workflow with RBAC

```
Day 0: Testnet staging
  └─ CanaryOperator tests on testnet (public role)

Day 1: Canary approval
  └─ Governance votes → Approves Stage 1 (canary)

Day 2: Canary deployment
  └─ CanaryOperator executes canary deployment
     (no Admin signature required)

Day 2-4: Canary monitoring
  └─ CanaryOperator monitors; reports to Governance

Day 4: Production approval
  └─ Governance votes → Approves Stage 2 (production)

Day 5: Production deployment
  └─ Admin (multisig) executes production rollout
     (requires threshold signatures)
```

---

## Access control audit trail

Log all RBAC-gated actions:

```json
{
  "deployment_id": "v6-2024-07-27",
  "timeline": [
    {
      "timestamp": "2024-07-27T09:00:00Z",
      "action": "upload_wasm",
      "actor": "canary_operator@karis-ky.dev",
      "wasm_hash": "abc123...",
      "status": "success"
    },
    {
      "timestamp": "2024-07-27T09:15:00Z",
      "action": "deploy_canary",
      "actor": "canary_operator@karis-ky.dev",
      "instances": ["CAAA...", "CBBB..."],
      "status": "success"
    },
    {
      "timestamp": "2024-07-28T12:00:00Z",
      "action": "canary_report_submitted",
      "actor": "canary_operator@karis-ky.dev",
      "canary_health": "OK",
      "error_rate": 0.05,
      "status": "submitted"
    },
    {
      "timestamp": "2024-07-28T14:00:00Z",
      "action": "governance_vote",
      "actor": "governance_multisig",
      "vote_result": "approved",
      "vote_count": "4/5",
      "status": "approved"
    },
    {
      "timestamp": "2024-07-28T15:00:00Z",
      "action": "deploy_production",
      "actor": "admin_multisig",
      "instances": ["CCCC...", "CDDD...", "..."],
      "signer_count": "3/5",
      "status": "success"
    }
  ]
}
```

---

## Onboarding a new CanaryOperator

**Prerequisites:**
- [ ] Passed security background check
- [ ] Acknowledged code of conduct
- [ ] Trained on canary procedures
- [ ] Signed NDA / confidentiality agreement

**Onboarding process:**

1. Generate new keypair:
   ```bash
   stellar account create-keypair
   # Save secret to secure storage
   ```

2. Admin adds to contract access control:
   ```bash
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --source $ADMIN_SECRET \
     --network mainnet \
     -- add_canary_operator \
     --operator G...
   ```

3. Fund keypair with XLM for transaction fees:
   ```bash
   stellar payment --destination G... --amount 100 --source $ADMIN_SECRET
   ```

4. Verify access:
   ```bash
   stellar contract invoke \
     --id <CANARY_INSTANCE> \
     --source $NEW_CANARY_OP_SECRET \
     --network mainnet \
     -- get_version
   ```

---

## Offboarding a CanaryOperator

**Trigger:** Turnover, rotation, or revocation

**Process:**

1. Notify governance (immediately)
2. Admin revokes access:
   ```bash
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --source $ADMIN_SECRET \
     --network mainnet \
     -- remove_canary_operator \
     --operator G...
   ```

3. Revoke keypair funding
4. Archive audit logs (for compliance)
5. Update access control documentation

---

## Separation of duties

### CanaryOperator cannot:
- ✗ Deploy to production instances
- ✗ Promote canary to production
- ✗ Change canary flags on instances
- ✗ Manage Admin access
- ✗ Escalate rollbacks beyond canary
- ✗ Clear legal hold on production

### Admin cannot:
- ✗ Deploy directly without governance approval
- ✗ Bypass canary stage in deployment workflow
- ✗ Unilaterally change instance flags
- ✗ Override CanaryOperator monitoring

### Governance cannot:
- ✗ Deploy contracts directly
- ✗ Manage operational keys
- ✗ Execute on-chain changes
- (Only votes and approves)

---

## Emergency escalation matrix

| Scenario | Who decides | Who executes | Approval |
|----------|---|---|---|
| **Canary bug detected** | CanaryOperator | CanaryOperator (rollback) | Notify Governance (async) |
| **Canary performance issue** | CanaryOperator | CanaryOperator (investigate) | Report to Governance (24h) |
| **Production bug detected** | Admin + Governance | Admin (rollback) | Emergency vote (1h) |
| **New CanaryOperator needed** | Admin | Admin (add) | Governance consent (24h) |
| **Compromise of CanaryOperator key** | Admin + Governance | Admin (revoke) | Emergency vote (immediate) |

---

## Audit and compliance

### Monthly access review

```
FOR EACH:
  ├─ CanaryOperator key rotation status
  ├─ Unused keys identified and revoked
  ├─ Admin key status (cold storage, escrow)
  ├─ All RBAC-gated actions logged
  └─ Governance signoff on access report
```

### Annual security audit

- [ ] All keys rotated since last audit
- [ ] Access control policy reviewed
- [ ] RBAC enforcement tested on contract
- [ ] Canary deployment history audited
- [ ] Incident response tested

---

## Reference: Access control configuration

Store this in a managed config file (git-tracked, reviewed):

```json
{
  "access_control": {
    "canary_operators": [
      {
        "name": "Alice (ops lead)",
        "address": "GAAA...",
        "added_date": "2024-07-01",
        "rotation_due": "2025-01-01"
      },
      {
        "name": "Bob (ops engineer)",
        "address": "GBBB...",
        "added_date": "2024-07-15",
        "rotation_due": "2025-01-15"
      }
    ],
    "admin_multisig": {
      "signers": [
        "GADMIN1",
        "GADMIN2",
        "GADMIN3",
        "GADMIN4",
        "GADMIN5"
      ],
      "threshold": 3,
      "cold_storage_location": "Vault A, Box 1"
    },
    "governance": {
      "multisig_account": "GGOV...",
      "voting_members": 5,
      "threshold": 3,
      "vote_quorum": 0.6
    }
  }
}
```

