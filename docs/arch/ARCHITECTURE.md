# Auto-Generated Architecture Documentation

_Generated from contract code analysis_

## Overview

The karis-ky escrow contract manages invoice-backed liquidity via role-based authorization, 
immutable state transitions, and bounded per-investor storage.

## Quick Navigation

- [Entity Relationships](entity-relationships.md) — data model and storage layout
- [Data Flow](data-flow.md) — token and authorization paths
- [State Machine](state-machine.md) — escrow lifecycle transitions
- [Module Structure](module-structure.md) — code organization
- [Entrypoint Matrix](entrypoint-matrix.md) — role-based API surface
- [Storage Reference](storage-reference.md) — detailed key/type catalog
## Key Metrics

- **DataKey variants:** 29
- **Contract types:** 6
- **Public entrypoints:** 67
- **Schema version:** 6 (current)
- **Max attestation entries:** 32
- **Max dust sweep per call:** 100M base units

## Architecture Decision Records

| ADR | Decision |
|-----|----------|
| ADR-001 | State model (status 0–4, forward-only transitions) |
| ADR-002 | Authorization boundaries (admin, SME, investor, treasury) |
| ADR-003 | Two-phase settlement + funding-close snapshot |
| ADR-004 | Legal/compliance hold mechanism |
| ADR-005 | Tiered yield with per-investor commitment locks |
| ADR-006 | Treasury dust sweep + SEP-41 token safety |
| ADR-007 | Per-investor keys in persistent storage (v6 migration) |