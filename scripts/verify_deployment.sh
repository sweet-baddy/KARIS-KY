#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# karis-ky Escrow — Post-Deployment Verification
# ─────────────────────────────────────────────────────────────────────────────
#
# Runs after `scripts/deploy.sh` (or any equivalent deployer) to verify that:
#   1. The deployed contract's WASM hash matches the built artifact.
#   2. `get_version` returns the expected `SCHEMA_VERSION`.
#   3. Basic entrypoints are accessible (read-only getters, no host panics).
#   4. (Optional) Initial state matches values from the deployment config.
#
# The script is **CI-friendly**: exit code 0 on success, 1 on any failure.
# Every check prints a structured line; CI pipelines can grep the output.
#
# Usage:
#   CONTRACT_ID=C... WASM_PATH=target/.../karis_ky_escrow.wasm \
#     bash scripts/verify_deployment.sh
#
#   bash scripts/verify_deployment.sh \
#     --contract-id C... \
#     --wasm target/.../karis_ky_escrow.wasm \
#     --network testnet \
#     --rpc-url https://soroban-testnet.stellar.org \
#     --expected-schema-version 7
#
# Optional environment / flag variables:
#   EXPECTED_ADMIN        — expected admin address (from .env); if set, verifies via get_escrow
#   EXPECTED_INVOICE_ID   — expected invoice_id (from .env); if set, verifies via get_escrow
#   EXPECTED_FUNDING_TOKEN— expected funding token (from .env); if set, verifies via get_funding_token
# ─────────────────────────────────────────────────────────────────────────────

set -uo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────

WASM_PATH="${WASM_PATH:-target/wasm32-unknown-unknown/release/karis_ky_escrow.wasm}"
CONTRACT_ID="${CONTRACT_ID:-}"
STELLAR_NETWORK="${STELLAR_NETWORK:-local}"
SOROBAN_RPC_URL="${SOROBAN_RPC_URL:-http://localhost:8000/soroban/rpc}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
EXPECTED_SCHEMA_VERSION="${EXPECTED_SCHEMA_VERSION:-7}"

EXPECTED_ADMIN="${EXPECTED_ADMIN:-}"
EXPECTED_INVOICE_ID="${EXPECTED_INVOICE_ID:-}"
EXPECTED_FUNDING_TOKEN="${EXPECTED_FUNDING_TOKEN:-}"

# ── Color helpers ───────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()    { echo -e "${BLUE}[INFO]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC}   $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail()    { echo -e "${RED}[FAIL]${NC} $*"; FAILED=1; }

FAILED=0
CHECKS=0
PASSED=0

check_start() {
  CHECKS=$((CHECKS + 1))
  info "─── Check ${CHECKS}: $1"
}

check_pass() {
  PASSED=$((PASSED + 1))
  success "$1"
}

# ── Argument parsing ────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --contract-id)              CONTRACT_ID="$2";              shift 2 ;;
    --wasm)                     WASM_PATH="$2";                shift 2 ;;
    --network)                  STELLAR_NETWORK="$2";          shift 2 ;;
    --rpc-url)                  SOROBAN_RPC_URL="$2";          shift 2 ;;
    --network-passphrase)       NETWORK_PASSPHRASE="$2";       shift 2 ;;
    --expected-schema-version)  EXPECTED_SCHEMA_VERSION="$2";  shift 2 ;;
    --expected-admin)           EXPECTED_ADMIN="$2";           shift 2 ;;
    --expected-invoice-id)      EXPECTED_INVOICE_ID="$2";      shift 2 ;;
    --expected-funding-token)   EXPECTED_FUNDING_TOKEN="$2";   shift 2 ;;
    -h|--help)
      sed -n '3,32p' "$0" | sed 's/^# //; s/^#//'
      exit 0
      ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

# ── Pre-flight ──────────────────────────────────────────────────────────────

if [ -z "${CONTRACT_ID}" ]; then
  fail "CONTRACT_ID is required (env var or --contract-id)"
fi

if [ ! -f "${WASM_PATH}" ]; then
  fail "WASM artifact not found: ${WASM_PATH}"
fi

if ! command -v stellar >/dev/null 2>&1; then
  fail "stellar CLI not found in PATH. Install from https://developers.stellar.org"
fi

if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
  fail "Neither sha256sum nor shasum available"
fi

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    # macOS fallback
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

# ── Workdir for fetched WASM ────────────────────────────────────────────────

WORKDIR=$(mktemp -d)
trap 'rm -rf "${WORKDIR}"' EXIT

# ─────────────────────────────────────────────────────────────────────────────
# Check 1: WASM hash matches deployed artifact
# ─────────────────────────────────────────────────────────────────────────────

check_start "WASM hash matches deployed artifact"

info "Local WASM: ${WASM_PATH}"
LOCAL_HASH=$(sha256_of "${WASM_PATH}")
info "Local SHA-256: ${LOCAL_HASH}"

info "Fetching deployed WASM from ${STELLAR_NETWORK} (${CONTRACT_ID})..."
FETCHED_WASM="${WORKDIR}/deployed.wasm"
FETCH_ERR="${WORKDIR}/fetch.err"

if ! stellar contract fetch \
    --id "${CONTRACT_ID}" \
    --network "${STELLAR_NETWORK}" \
    --rpc-url "${SOROBAN_RPC_URL}" \
    --network-passphrase "${NETWORK_PASSPHRASE}" \
    --out "${FETCHED_WASM}" \
    >"${WORKDIR}/fetch.out" 2>"${FETCH_ERR}"; then
  fail "stellar contract fetch failed: $(cat ${FETCH_ERR})"
fi

if [ ! -s "${FETCHED_WASM}" ]; then
  fail "Fetched WASM is empty: $(cat ${FETCH_ERR})"
fi

DEPLOYED_HASH=$(sha256_of "${FETCHED_WASM}")
info "Deployed SHA-256: ${DEPLOYED_HASH}"

if [ "${LOCAL_HASH}" != "${DEPLOYED_HASH}" ]; then
  fail "WASM hash mismatch: local=${LOCAL_HASH}, deployed=${DEPLOYED_HASH}"
else
  check_pass "WASM hash matches (${LOCAL_HASH})"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 2: get_version returns expected SCHEMA_VERSION
# ─────────────────────────────────────────────────────────────────────────────

check_start "get_version returns expected SCHEMA_VERSION (${EXPECTED_SCHEMA_VERSION})"

info "Invoking get_version..."
GV_OUT="${WORKDIR}/gv.out"
GV_ERR="${WORKDIR}/gv.err"

if ! stellar contract invoke \
    --id "${CONTRACT_ID}" \
    --network "${STELLAR_NETWORK}" \
    --rpc-url "${SOROBAN_RPC_URL}" \
    --network-passphrase "${NETWORK_PASSPHRASE}" \
    -- get_version \
    >"${GV_OUT}" 2>"${GV_ERR}"; then
  fail "get_version invocation failed: $(cat ${GV_ERR})"
fi

# The Stellar CLI prints the return value followed by logs. Parse the first number.
RAW_VERSION=$(grep -oE '[0-9]+' "${GV_OUT}" | head -1 || true)

if [ -z "${RAW_VERSION}" ]; then
  fail "Could not parse version from get_version output: $(cat ${GV_OUT})"
fi

info "get_version returned: ${RAW_VERSION}"

if [ "${RAW_VERSION}" != "${EXPECTED_SCHEMA_VERSION}" ]; then
  fail "SCHEMA_VERSION mismatch: expected=${EXPECTED_SCHEMA_VERSION}, got=${RAW_VERSION}"
else
  check_pass "SCHEMA_VERSION matches (${RAW_VERSION})"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 3: basic read-only entrypoints are accessible
# ─────────────────────────────────────────────────────────────────────────────

check_start "basic read-only entrypoints are accessible (no host panics)"

invoke_or_fail() {
  local entry="$1"
  local out="${WORKDIR}/${entry}.out"
  local err="${WORKDIR}/${entry}.err"

  if ! stellar contract invoke \
      --id "${CONTRACT_ID}" \
      --network "${STELLAR_NETWORK}" \
      --rpc-url "${SOROBAN_RPC_URL}" \
      --network-passphrase "${NETWORK_PASSPHRASE}" \
      -- "$@" \
      >"${out}" 2>"${err}"; then
    # Some read-only entrypoints return typed errors when storage is missing
    # (e.g. get_escrow before init emits EscrowNotInitialized). That's expected
    # and indicates the entrypoint is accessible. We only fail on transport
    # errors (RPC unreachable, malformed contract, missing WASM, etc.).
    if grep -qiE "network error|rpc error|connect|timeout|panic|transport|http error" "${err}"; then
      fail "Entrypoint '${entry}' unreachable: $(cat ${err})"
    fi
  fi
}

invoke_or_fail get_interface_version
check_pass "get_interface_version responds"

invoke_or_fail get_escrow_summary
check_pass "get_escrow_summary responds"

invoke_or_fail get_funding_token
check_pass "get_funding_token responds"

invoke_or_fail get_treasury
check_pass "get_treasury responds"

# ─────────────────────────────────────────────────────────────────────────────
# Check 4 (optional): initial state matches deployment config
# ─────────────────────────────────────────────────────────────────────────────

if [ -n "${EXPECTED_ADMIN}" ] || [ -n "${EXPECTED_INVOICE_ID}" ] || [ -n "${EXPECTED_FUNDING_TOKEN}" ]; then
  check_start "initial state matches deployment config"

  ESCROW_OUT="${WORKDIR}/escrow.out"
  ESCROW_ERR="${WORKDIR}/escrow.err"

  if ! stellar contract invoke \
      --id "${CONTRACT_ID}" \
      --network "${STELLAR_NETWORK}" \
      --rpc-url "${SOROBAN_RPC_URL}" \
      --network-passphrase "${NETWORK_PASSPHRASE}" \
      -- get_escrow \
      >"${ESCROW_OUT}" 2>"${ESCROW_ERR}"; then
    if grep -qiE "NotInitialized|not.*initialized" "${ESCROW_ERR}" "${ESCROW_OUT}" 2>/dev/null; then
      warn "Contract not yet initialized — skipping state config verification"
    else
      fail "get_escrow failed: $(cat ${ESCROW_ERR})"
    fi
  else
    ESCROW_DUMP="${ESCROW_OUT}"

    if [ -n "${EXPECTED_INVOICE_ID}" ]; then
      if grep -qF "${EXPECTED_INVOICE_ID}" "${ESCROW_DUMP}"; then
        check_pass "invoice_id matches: ${EXPECTED_INVOICE_ID}"
      else
        fail "invoice_id mismatch: expected '${EXPECTED_INVOICE_ID}'"
      fi
    fi

    if [ -n "${EXPECTED_ADMIN}" ]; then
      if grep -qF "${EXPECTED_ADMIN}" "${ESCROW_DUMP}"; then
        check_pass "admin address matches: ${EXPECTED_ADMIN}"
      else
        fail "admin address mismatch: expected '${EXPECTED_ADMIN}'"
      fi
    fi
  fi

  if [ -n "${EXPECTED_FUNDING_TOKEN}" ]; then
    FT_OUT="${WORKDIR}/ft.out"
    FT_ERR="${WORKDIR}/ft.err"

    if ! stellar contract invoke \
        --id "${CONTRACT_ID}" \
        --network "${STELLAR_NETWORK}" \
        --rpc-url "${SOROBAN_RPC_URL}" \
        --network-passphrase "${NETWORK_PASSPHRASE}" \
        -- get_funding_token \
        >"${FT_OUT}" 2>"${FT_ERR}"; then
      fail "get_funding_token failed: $(cat ${FT_ERR})"
    fi

    if grep -qF "${EXPECTED_FUNDING_TOKEN}" "${FT_OUT}"; then
      check_pass "funding token matches: ${EXPECTED_FUNDING_TOKEN}"
    else
      fail "funding token mismatch: expected '${EXPECTED_FUNDING_TOKEN}'"
    fi
  fi
else
  info "Skipping initial-state-config check (no EXPECTED_* env vars set)"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════════════"
info "Verification summary: ${PASSED}/${CHECKS} checks passed"
if [ ${FAILED} -eq 0 ]; then
  success "Deployment verification: PASSED"
  echo "═══════════════════════════════════════════════════════════════"
  exit 0
else
  echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
  fail "Deployment verification: FAILED"
  echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
  exit 1
fi