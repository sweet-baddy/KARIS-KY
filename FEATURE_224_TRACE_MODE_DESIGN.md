# Feature #224: Contract Debugger Trace Mode

## Issue Specification

**Issue ID:** FEAT-015
**Category:** FEAT
**Status:** Specified - Ready for assignment
**Related backlog item:** FEATURE_224
**Priority:** Medium

### Full Description

Add an opt-in trace mode to the escrow contract so operators and developers can
diagnose storage mutations and important state transitions from indexed contract
events. Trace data must be useful for forensic debugging without exposing stored
values, changing contract behavior, or adding meaningful overhead to the normal
production path.

The feature covers trace configuration, event emission, bounded in-contract
inspection, and tests for both enabled and disabled builds. It does not include a
new external observability service, historical backfilling, or a replacement for
the existing business events.

### Current Behavior

The contract emits business-domain events, but it has no trace-specific event
type, trace configuration entrypoint, trace buffer, or `trace-mode` Cargo feature.
An operator investigating a failed fund, settle, or claim must infer internal
storage activity from the transaction result and existing business events.

### Steps to Reproduce

1. Build or test the workspace using the default Cargo features.
2. Execute a representative escrow operation such as `fund`, `settle`, or
   `claim`.
3. Inspect the emitted events and contract instance storage.
4. Attempt to enable tracing or retrieve a trace buffer.

**Observed result:** Only existing business events are available; there is no
trace configuration or read/write history to inspect. The repository also has no
`trace-mode` feature or trace-specific implementation.

### Expected vs. Actual Behavior

| Area | Expected behavior | Actual behavior |
|------|-------------------|-----------------|
| Configuration | An authorized admin can enable a documented level and disable tracing. | No trace configuration API exists. |
| Diagnostics | Enabled tracing emits structured events for selected storage operations and state transitions. | No trace events are emitted. |
| Data exposure | Trace payloads contain hashes and metadata, not raw stored values. | No trace payload contract exists. |
| Production path | Tracing is off by default and disabled builds preserve current behavior and overhead. | No opt-in trace feature or no-op path exists. |
| Inspection | Operators can read and clear a bounded trace buffer when enabled. | No trace buffer exists. |

### Environment Context

- **Repository:** `sweet-baddy/KARIS-KY`
- **Branch:** `main`
- **Runtime:** Soroban smart contract in Rust
- **Workspace manifest:** `Cargo.toml` with the `escrow` member
- **Primary implementation surface:** `escrow/src/lib.rs`
- **Relevant operations:** funding, settlement, claims, storage reads/writes, and
  state transitions
- **Reproduction date:** 2026-08-27
- **Network/deployment:** Reproducible locally; no deployed network is required
  to verify the missing capability

### Proposed Solution

1. Add a `trace-mode` Cargo feature and compile trace instrumentation behind it.
2. Define versioned, structured trace event and buffer types containing the
   verbosity level, operation name, key/value hashes, ledger timestamp, and a
   bounded diagnostic message.
3. Add admin-only enable, disable, and clear entrypoints, plus a read-only
   buffer inspection entrypoint. Reject unsupported levels with a typed error.
4. Instrument the smallest shared storage/state-transition helpers so fund,
   settle, and claim paths produce consistent traces without duplicating logic.
5. Keep tracing disabled by default. In builds without `trace-mode`, compile
   instrumentation to no-ops and retain existing business-event behavior.
6. Add unit and integration tests for authorization, level filtering, hash-only
   payloads, buffer bounds, clearing, disabled behavior, and compatibility with
   existing operations.

### Acceptance Criteria

- [ ] `trace-mode` is an optional Cargo feature and is disabled by default.
- [ ] An admin can enable each supported level and disable tracing; unauthorized
    callers and invalid levels fail with typed errors.
- [ ] Enabled tracing emits structured events for the documented storage writes
    and state transitions in `fund`, `settle`, and `claim`.
- [ ] TRACE-level reads and DEBUG-level writes are filtered according to the
    documented verbosity table; lower levels do not emit higher-level details.
- [ ] Trace payloads never contain raw storage keys or raw stored values; hashes
    and bounded metadata are used instead.
- [ ] The trace buffer has a fixed maximum size, remains readable, and can be
    cleared by an admin without affecting escrow state.
- [ ] Tracing is off by default, and existing business events and return values
    are unchanged when it is disabled.
- [ ] Tests cover enabled, disabled, unauthorized, invalid-level, filtering,
    buffer-bound, clear, and representative escrow-operation cases.
- [ ] `cargo test` and `cargo test --features trace-mode` pass for the workspace.
- [ ] Operator documentation explains configuration, event fields, retention,
    and the production/staging/development recommended levels.

## Overview

Emit detailed trace events for each storage operation (read/write) and state transitions for forensic analysis and debugging.

## Design Decisions

### 1. Architecture

**Two-tier tracing:**
- **Event-based:** Critical events (state transitions, errors) published via `#[contractevent]`
- **Buffer-based:** Detailed operation logs stored in circular buffer (instance storage)

**Why two tiers?**
- Events are queryable via Soroban indexer
- Buffer logs are fast, bounded, and can be inspected via REPL
- Users choose verbosity; info-level events are always-on, trace-level only in debug mode

### 2. Verbosity Levels

```
OFF      = No tracing (production default)
ERROR    = Only errors and failures
WARN     = Errors + state transition warnings
INFO     = Warn + significant method calls (fund, settle, claim)
DEBUG    = Info + all storage writes
TRACE    = Debug + all storage reads + internal calculations
```

### 3. Storage Overhead

- Circular buffer: ~1000 entries × 256 bytes = 256 KB (bounded)
- Per-entry: `{ operation, key_hash, value_hash, ts }`
- Hashes used instead of full values to limit storage
- Can be cleared via admin entrypoint

## Implementation

### 3.1 New Types (in lib.rs)

```rust
#[contractevent]
pub struct TraceEvent {
    pub level: u32,        // 0=OFF, 1=ERROR, 2=WARN, 3=INFO, 4=DEBUG, 5=TRACE
    pub operation: String, // "StorageRead", "StorageWrite", "FunctionEnter", "StateTransition"
    pub key_hash: [u8; 32],
    pub old_value_hash: [u8; 32],
    pub new_value_hash: [u8; 32],
    pub timestamp: u64,
    pub message: String,
}

#[contracttype]
pub struct TraceBuffer {
    pub entries: Vec<TraceEntry>,
    pub level: u32,
    pub enabled: bool,
}

#[contracttype]
pub struct TraceEntry {
    pub operation: Symbol,       // "read", "write", "enter", "exit", "transition"
    pub key_name: Symbol,        // "Escrow", "InvestorContribution", etc.
    pub value_hash: [u8; 32],
    pub ts: u64,
    pub msg: String,
}

pub enum DataKey {
    // ... existing variants ...
    TraceBuffer,
    TraceLevel,
}
```

### 3.2 Integration Points

**Storage wrapper macros (optional optimization):**

```rust
macro_rules! trace_get {
    ($env:expr, $key:expr) => {{
        let result = $env.storage().instance().get(&$key);
        if let Some(level) = get_trace_level(&$env) {
            if level >= TRACE_LEVEL {
                emit_trace_read(&$env, &$key, &result);
            }
        }
        result
    }};
}

macro_rules! trace_set {
    ($env:expr, $key:expr, $val:expr) => {{
        let old = $env.storage().instance().get(&$key);
        $env.storage().instance().set(&$key, &$val);
        if let Some(level) = get_trace_level(&$env) {
            if level >= DEBUG_LEVEL {
                emit_trace_write(&$env, &$key, &old, Some(&$val));
            }
        }
    }};
}
```

**State transition hook:**

```rust
fn mark_state_transition(env: Env, from: u32, to: u32, reason: &str) {
    if let Some(level) = get_trace_level(&env) {
        if level >= WARN_LEVEL {
            TraceEvent {
                level: WARN_LEVEL,
                operation: symbol_short!("state_change"),
                key_hash: [0u8; 32], // Unused
                old_value_hash: encode_u32_as_hash(from),
                new_value_hash: encode_u32_as_hash(to),
                timestamp: env.ledger().timestamp(),
                message: String::from_small(reason),
            }.publish(&env);
        }
    }
}
```

### 3.3 Entrypoints

```rust
/// Enable tracing at specified level. Admin-only.
pub fn enable_tracing(env: Env, level: u32) {
    let escrow = Self::load_escrow_require_admin(&env);
    ensure(&env, level <= TRACE_LEVEL, EscrowError::InvalidTraceLevel);
    
    env.storage().instance().set(&DataKey::TraceLevel, &level);
    
    event::TraceConfigured {
        level,
        timestamp: env.ledger().timestamp(),
    }.publish(&env);
}

/// Disable tracing. Admin-only.
pub fn disable_tracing(env: Env) {
    Self::load_escrow_require_admin(&env);
    env.storage().instance().remove(&DataKey::TraceLevel);
}

/// Read current trace buffer (permissionless, read-only).
pub fn get_trace_buffer(env: Env) -> Vec<TraceEntry> {
    env.storage()
        .instance()
        .get::<DataKey, TraceBuffer>(&DataKey::TraceBuffer)
        .map(|tb| tb.entries)
        .unwrap_or_default()
}

/// Clear trace buffer. Admin-only.
pub fn clear_trace_buffer(env: Env) {
    Self::load_escrow_require_admin(&env);
    env.storage().instance().remove(&DataKey::TraceBuffer);
}
```

### 3.4 Feature Flag

Add to `Cargo.toml`:
```toml
[features]
trace-mode = []
testutils = ["soroban-sdk/testutils"]

[dev-dependencies]
# Only trace macros/wrappers compiled in test builds
```

Use in code:
```rust
#[cfg(feature = "trace-mode")]
fn emit_trace_read(...) { ... }

#[cfg(not(feature = "trace-mode"))]
fn emit_trace_read(...) { /* no-op */ }
```

## Testing

### Unit Tests

```rust
#[test]
fn test_trace_mode_on_fund() {
    let (env, client, investor, _) = setup_with_tracing(DEBUG_LEVEL);
    
    client.fund(&investor, 1000);
    
    let traces = client.get_trace_buffer();
    // Assert traces contain:
    // - StorageRead: Escrow
    // - StorageWrite: InvestorContribution
    // - StateTransition: 0 -> 1 (if funded)
}

#[test]
fn test_trace_disabled_no_overhead() {
    let (env, client, investor, _) = setup_with_tracing(OFF_LEVEL);
    
    // Should have minimal overhead
    for i in 0..1000 {
        client.fund(&investor, 1);
    }
    
    let traces = client.get_trace_buffer();
    assert!(traces.is_empty());
}
```

## Performance Considerations

- **Disabled (default):** ~0% overhead (no-op macro)
- **WARN level:** <1% overhead (events only)
- **DEBUG level:** ~5% overhead (write tracking)
- **TRACE level:** ~15% overhead (all reads/writes)

## Deployment Notes

- **Production:** Keep trace level at OFF (default)
- **Staging:** Use DEBUG level for incident investigation
- **Development:** Use TRACE level freely

## Migration

No schema version bump required. Trace buffer is optional; missing traces are treated as "no history available."

