# [SECURITY] Audit Pre-Flight-Checklist.sh for Secret Exposure in Environment Variables

## Issue Summary

The `scripts/pre-flight-checklist.sh` and related deployment scripts (`deploy.sh`) handle sensitive secrets (Stellar private keys) via environment variables with **insufficient protections against exposure**. Secrets can be leaked through:

- Process argument inspection (`ps`, `/proc`)
- Shell history (`.bash_history`, `.zsh_history`)
- Debug/verbose logging
- Error messages and stack traces
- Script dumps in CI/CD logs
- Accidental commits to version control

**Severity:** CRITICAL (secrets are plaintext in memory; can compromise mainnet deployments)  
**Status:** Design audit required  
**Affected files:**
- `scripts/pre-flight-checklist.sh` (loads secrets, checks for presence)
- `scripts/deploy.sh` (passes secrets to stellar CLI)
- `scripts/verify_deployment.sh` (may log environment state)
- `scripts/local-env.sh` (local development, but sets secrets in shell)
- `.env*` files (committed to repo as placeholders, but documentation inadequate)

---

## Problem Description

### Current Implementation

The scripts currently:

1. **Load secrets from .env file**
```bash
if [ -f "${ENV_FILE}" ]; then
  set -a
  source "${ENV_FILE}"
  set +a
fi

SOURCE_SECRET="${SOURCE_SECRET:-}"
DEPLOYER_ADDRESS="${DEPLOYER_ADDRESS:-}"
```

2. **Check if secrets are present** (but don't mask them)
```bash
[ -z "$SOURCE_SECRET" ] && MISSING_VARS+=("SOURCE_SECRET")
```

3. **Pass secrets directly to CLI tools**
```bash
stellar contract deploy \
  --secret-key "${SOURCE_SECRET}" \
  ...
```

### Vulnerability Vectors

#### 1. **Process Argument Inspection (CRITICAL)**

Any user on the system can inspect running processes:
```bash
# Attacker observes:
ps aux | grep stellar
# Output shows:
user  12345  stellar contract deploy --secret-key Sxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

**Impact:** Secret visible to any system user, including unprivileged containers

#### 2. **Shell History (CRITICAL)**

If script is run manually in a shell, secrets appear in history:
```bash
# User runs:
bash scripts/deploy.sh --env .env.mainnet

# Secret stored in:
~/.bash_history
~/.zsh_history
# Even if user doesn't run it interactively, bash_history may capture it
```

**Impact:** Persistent storage of secrets on disk

#### 3. **Script Dump in CI/CD (CRITICAL)**

CI/CD systems may log full script execution:
```bash
# GitHub Actions / GitLab CI logs show:
[DEBUG] Running: stellar contract deploy --secret-key Sxxxxxxxxxxxxxxxx
```

**Impact:** Secrets visible in CI logs, often retained for months

#### 4. **Error Messages (HIGH)**

Deployment failures may include command in error message:
```bash
Error executing: stellar contract deploy --secret-key Sxxx... --wasm ...
```

**Impact:** Secrets in error logs, support tickets, stack traces

#### 5. **Child Process Leaks (MEDIUM)**

Subprocess spawned by scripts may inherit environment:
```bash
# If script forks before clearing env:
execvp("stellar", argv) // argv contains secret if not sanitized
```

**Impact:** Secret visible to subprocesses and debuggers

#### 6. **.env Files in Version Control (MEDIUM)**

Repository contains placeholder .env files:
```bash
# .env.mainnet and .env.testnet are committed to repo
# If secrets are accidentally added here, they're:
# - Permanently in git history
# - Visible to all repo members
# - Available if repo is leaked
```

**Impact:** Secrets may be checked in accidentally during maintenance

---

## Steps to Reproduce

### Scenario 1: Inspect Running Process

**Precondition:** `scripts/deploy.sh` is running

**Steps:**
1. In another terminal: `ps aux | grep stellar`
2. Observe output showing full command line with secret key visible

**Result:** ❌ FAIL — Secret exposed in process listing

### Scenario 2: Check Shell History

**Precondition:** User runs `bash scripts/deploy.sh --env .env.mainnet` in interactive shell

**Steps:**
1. After script finishes, press up arrow in shell history
2. Or: `cat ~/.bash_history | grep SOURCE_SECRET`
3. Or: `history | grep secret`

**Result:** ❌ FAIL — Secret visible in shell history file

### Scenario 3: Environment Variable Dump

**Precondition:** Script is modified to show all env vars

**Steps:**
1. Add `env | grep SECRET` to script
2. Run script
3. Observe output

**Result:** ❌ FAIL — All secrets displayed in plaintext

### Scenario 4: CI/CD Log Leak

**Precondition:** Script is run in CI/CD with full logging enabled

**Steps:**
1. Push changes to trigger CI pipeline
2. Enable debug logging (GitHub Actions: `ACTIONS_RUNNER_DEBUG=true`)
3. View build logs

**Result:** ❌ FAIL — Secrets visible in CI logs

---

## Expected vs. Actual Behavior

| Scenario | Expected | Actual | Gap |
|----------|----------|--------|-----|
| **Process inspection** | Secret masked/hidden from `ps` | Visible in full command line | CRITICAL |
| **Shell history** | Not stored in history | Stored in ~/.bash_history | CRITICAL |
| **Error messages** | Secrets redacted/masked | Full secrets in error output | CRITICAL |
| **Environment dumps** | Secrets not displayed | Displayed in plaintext via `env` | CRITICAL |
| **Child processes** | Secrets cleared before exec | Inherited by subprocesses | MEDIUM |
| **Version control** | Secrets never committed | Placeholders OK but guidance weak | MEDIUM |
| **CI/CD logs** | Secrets masked in logs | Full output visible if debug enabled | CRITICAL |
| **Memory dumps** | Process memory protected by OS | Memory readable by root/debuggers | LOW (host-level) |

---

## Proposed Solution

Implement comprehensive secret handling hardening with **three layers**:

### Layer 1: Input Protection

#### 1a. Remove secrets from shell history
```bash
# At script start:
if [ -n "${BASH_VERSION:-}" ]; then
  export HISTCONTROL=ignorespace
  # Or set HISTFILE=/dev/null for this script
fi
```

#### 1b. Validate .env file permissions
```bash
if [ -f "${ENV_FILE}" ]; then
  # Check file is not world-readable (contains secrets)
  FILE_PERMS=$(stat -c "%a" "${ENV_FILE}" 2>/dev/null || stat -f "%A" "${ENV_FILE}" 2>/dev/null)
  if [[ "$FILE_PERMS" == *"4"* ]] || [[ "$FILE_PERMS" == *"2"* ]]; then
    warn ".env file is readable by others (world/group readable)"
    fail "Fix permissions: chmod 600 ${ENV_FILE}"
  fi
fi
```

#### 1c. Source .env file carefully
```bash
# Instead of:
source "${ENV_FILE}"

# Use:
# 1. Read into subshell to isolate
# 2. Export only needed vars
# 3. Clear rest
set -a
source "${ENV_FILE}"
set +a

# OR: Read vars explicitly
SOURCE_SECRET=$(grep '^SOURCE_SECRET=' "${ENV_FILE}" | cut -d= -f2 | tr -d "\"'")
DEPLOYER_ADDRESS=$(grep '^DEPLOYER_ADDRESS=' "${ENV_FILE}" | cut -d= -f2 | tr -d "\"'")
```

### Layer 2: Processing Protection

#### 2a. Mask secrets in variable expansion
```bash
# Define a masking function
redact() {
  if [ ${#1} -gt 4 ]; then
    echo "${1:0:2}...${1: -2}"
  else
    echo "***"
  fi
}

# When logging/checking vars:
info "DEPLOYER_ADDRESS: $DEPLOYER_ADDRESS"  # ✓ Public key is OK
info "SOURCE_SECRET: $(redact "$SOURCE_SECRET")"  # ✓ Masked
```

#### 2b. Use file descriptor instead of command-line args
```bash
# Instead of passing secret as argument:
stellar contract deploy --secret-key "${SOURCE_SECRET}" ...

# Use file descriptor or stdin:
stellar contract deploy --secret-key /dev/fd/3 ... 3<<"EOF"
${SOURCE_SECRET}
EOF

# OR use environment var (cleaner):
export STELLAR_SECRET_KEY="${SOURCE_SECRET}"
stellar contract deploy ...  # reads from env, not args
```

#### 2c. Implement strict error handling
```bash
# Instead of default error messages:
trap 'error_handler' EXIT

error_handler() {
  # Don't dump environment or command state
  # Only log what's safe
  if [ $? -ne 0 ]; then
    fail "Deployment failed (check logs for details)"
    # Clear sensitive vars before exit
    unset SOURCE_SECRET
    unset DEPLOYER_ADDRESS
  fi
}
```

### Layer 3: Output Protection

#### 3a. Mask secrets in all output
```bash
# Create a filter function:
mask_output() {
  local output="$1"
  # Mask Stellar secret keys (start with 'S', 56 chars)
  output=$(echo "$output" | sed -E 's/S[A-Z0-9]{55}/S...MASKED.../g')
  # Mask contract IDs (start with 'C', 56 chars)
  output=$(echo "$output" | sed -E 's/C[A-Z0-9]{55}/C...MASKED.../g')
  echo "$output"
}

# Use when logging:
echo "$(mask_output "$cli_output")"
```

#### 3b. Clear variables on exit
```bash
cleanup() {
  # Explicitly clear all secrets before exit
  unset SOURCE_SECRET
  unset DEPLOYER_ADDRESS
  unset NETWORK_PASSPHRASE
  # For additional safety:
  shred -vfz -n 3 <<<"$SOURCE_SECRET" 2>/dev/null || true
}

trap cleanup EXIT
```

#### 3c. Disable debug mode
```bash
# At script start, explicitly disable debug output
set +x  # Disable command echoing (even if -x was used)

# Ensure history substitution doesn't leak:
set +H  # Disable history expansion in non-interactive shells
```

---

## Acceptance Criteria

### AC1: Input Validation & Permissions

- [ ] `pre-flight-checklist.sh` validates .env file is not world/group-readable
- [ ] Fails with clear error if permissions are 644 or broader
- [ ] Documentation warns operators to use `chmod 600 .env*`

### AC2: Secret Masking in Output

- [ ] All secret key references (starting with 'S', 56 chars) are masked as `S...MASKED...` in output
- [ ] Contractor addresses (starting with 'C', 56 chars) are masked as `C...MASKED...`
- [ ] Masking works in info/warn/fail/ok functions

### AC3: History Protection

- [ ] Scripts set `HISTCONTROL=ignorespace` when in bash interactive shell
- [ ] Commands starting with space are not recorded in history
- [ ] OR: Scripts use `HISTFILE=/dev/null` to disable history entirely

### AC4: Process Argument Protection

- [ ] Secrets are NOT passed as command-line arguments to external tools
- [ ] Instead: use environment variables or stdin/file descriptors
- [ ] `stellar contract deploy` called WITHOUT `--secret-key` on command line

### AC5: Error Handler Hardening

- [ ] Explicit error handler clears secrets before exit
- [ ] Error messages do NOT include full command line or environment dump
- [ ] Secrets unset before any exit path

### AC6: CI/CD Integration Guide

- [ ] Documentation explains how to mask secrets in CI logs
- [ ] Example provided for GitHub Actions / GitLab CI
- [ ] Instructions to use `::add-mask::` (GitHub) or CI masking equivalent

### AC7: .env File Security Guide

- [ ] README/documentation explains `.env.local` pattern (not committed)
- [ ] Checklist: "Before committing, verify no real secrets in .env files"
- [ ] Git hooks (pre-commit) that detect secret key patterns (S-prefix + length)

### AC8: Testing & Verification

- [ ] Unit test: `test_env_file_permissions_checked` — fails if 644 or broader
- [ ] Unit test: `test_secrets_masked_in_output` — verifies redaction
- [ ] Manual test: `ps aux` during deploy shows no secret keys
- [ ] Manual test: History file does not contain secret keys

### AC9: Documentation Updates

- [ ] `docs/DEPLOYER_SECURITY.md` (new or update `OPERATOR_RUNBOOK.md`)
  - Secret handling best practices
  - File permissions & .env.local pattern
  - CI/CD masking setup
  - Incident response (if secret exposed)
- [ ] Script comments updated with security notes

### AC10: Backward Compatibility

- [ ] Changes are backward compatible (no breaking CLI changes)
- [ ] Existing `.env` files still work (with permission warning)
- [ ] Scripts still pass all acceptance criteria without changes

---

## Implementation Checklist

- [ ] Add file permission check to `pre-flight-checklist.sh`
- [ ] Add secret masking function to scripts
- [ ] Update environment loading to use safer patterns
- [ ] Implement explicit cleanup/unset handlers
- [ ] Disable history recording (HISTCONTROL / HISTFILE)
- [ ] Update error handling (no env dump)
- [ ] Add masking to all output functions (info/warn/ok/fail)
- [ ] Update `deploy.sh` to use safer argument passing (env vars or stdin)
- [ ] Create `docs/DEPLOYER_SECURITY.md` with comprehensive guide
- [ ] Add `.pre-commit-hooks.yaml` to detect S-prefix secrets
- [ ] Add unit tests for permission checking and masking
- [ ] Add manual test procedure (ps, history, CI simulation)
- [ ] CI verification: cargo clippy, shellcheck, manual tests

---

## Security Threat Model

### Threat Actors

| Actor | Access Level | Threat |
|-------|--|---------|
| **Shared server admin** | Root/full access | Can dump process memory, inspect running commands |
| **Container escape** | Local user | Can inspect `ps`, environment, shared memory |
| **CI/CD system admin** | Read logs | Can view all job output including secrets |
| **Malicious script** | Subprocess | Inherits environment unless explicitly cleared |
| **Curious developer** | Local shell | Can view history, running processes |
| **Git history scanner** | Repo access | Can find secrets in committed files |

### Attack Scenarios

1. **Attacker gains temporary access to CI/CD system**
   - Views old build logs → finds `--secret-key S...` in output
   - Uses secret to deploy fraudulent contract or withdraw funds

2. **Developer shares shell session screenshot**
   - Screenshot shows `ps aux` output with visible secret
   - Secret extracted from screenshot and used maliciously

3. **Malicious script executes**
   - Reads `/proc/self/environ` or parent process environment
   - Extracts `SOURCE_SECRET` and exfiltrates to attacker

4. **Git history search**
   - Attacker clones repo and searches for secret key patterns
   - If secrets were ever committed, they're recoverable

### Mitigations

| Threat | Mitigation | Layer |
|--------|-----------|-------|
| Process inspection | Don't pass secrets as args (use env vars or stdin) | 2 |
| History leak | Disable history or mask commands | 1 |
| CI log leak | Mask secrets in output, redact error messages | 3 |
| Env dump | Don't echo $SOURCE_SECRET, unset on exit | 2 |
| File permissions | Validate .env not world-readable | 1 |
| Git leak | Pre-commit hook to detect secret patterns | 1 |

---

## Implementation Guidance

### Priority 1: Critical Fixes (Do First)

1. **Stop passing secrets as command-line arguments**
   - Update `deploy.sh` to use env vars or stdin
   - Requires changes to how `stellar` CLI is invoked

2. **Add file permission validation**
   - Check .env file is 600 or narrower
   - Fail deployment if not

3. **Mask secrets in all output**
   - Add masking function for S-prefixed keys
   - Apply to all logging

### Priority 2: High-Priority Fixes (Do Next)

4. **Disable history recording**
   - Set `HISTCONTROL=ignorespace` or `HISTFILE=/dev/null`
   - Add comments explaining why

5. **Implement cleanup handler**
   - Explicit `unset` of all secrets
   - Called on all exit paths

6. **Update error handling**
   - Remove command dump from error messages
   - Generic "deployment failed" message only

### Priority 3: Supporting Items (Do With Deployment)

7. **Create security guide document**
   - Best practices for secret handling
   - .env.local pattern explanation
   - Incident response procedure

8. **Add pre-commit hook**
   - Detect S-prefix patterns (56-char secret keys)
   - Warn before committing

9. **Add tests**
   - Permission checking
   - Masking verification
   - History file inspection

---

## Testing Strategy

### Unit Tests

#### Test 1: File Permission Validation
```bash
test_env_file_permissions_checked() {
  # Create .env with 644 (bad)
  touch test.env
  chmod 644 test.env
  
  # Script should fail
  ! bash scripts/pre-flight-checklist.sh --env test.env
  
  # Create .env with 600 (good)
  chmod 600 test.env
  bash scripts/pre-flight-checklist.sh --env test.env || true
}
```

#### Test 2: Secret Masking
```bash
test_secrets_masked_in_output() {
  SECRET="Sxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  
  # Mock info function
  info() {
    local msg="$1"
    msg=$(mask_output "$msg")
    echo "$msg"
  }
  
  OUTPUT=$(info "Secret is $SECRET")
  [[ "$OUTPUT" == *"S...MASKED..."* ]]
  [[ "$OUTPUT" != *"Sxxxxx"* ]]  # Should NOT contain original
}
```

#### Test 3: History Protection
```bash
test_history_disabled_during_execution() {
  # Run script with HISTFILE tracking
  HISTFILE=/tmp/test_history
  bash scripts/pre-flight-checklist.sh --env .env.testnet || true
  
  # History file should be empty or not created
  [ ! -f "$HISTFILE" ] || [ ! -s "$HISTFILE" ]
}
```

### Manual Verification Tests

#### Test 4: Process Inspection
```bash
# Run deploy in background
timeout 30 bash scripts/deploy.sh --env .env.testnet &
PID=$!

# Inspect process
sleep 1
ps -p $PID -o args=

# Should NOT show SECRET key, only masked or hidden
```

#### Test 5: CI/CD Log Masking
```bash
# Simulate CI/CD environment
export GITHUB_ACTIONS=true
bash scripts/pre-flight-checklist.sh --env .env.testnet 2>&1 | tee ci.log

# Verify log file
! grep -E 'S[A-Z0-9]{55}' ci.log  # Should have no plain secrets
```

---

## Error Code (Optional)

While this issue doesn't require new error codes, if adding status checks:

```rust
// Not needed for shell scripts, but documented for reference:
// EscrowError::SecretExposureDetected = 95  (if validation moves to contract)
```

---

## Documentation Updates

### 1. New: `docs/DEPLOYER_SECURITY.md`

**Sections:**
- Secret handling best practices
- .env file structure and permissions
- .env.local pattern (not committed)
- CI/CD masking setup (GitHub Actions, GitLab CI, etc.)
- Common pitfalls and how to avoid them
- Incident response (secret was exposed, what to do)
- Command-line tool integration guidance

### 2. Update: `README.md`

Add warning:
```markdown
⚠️ **SECRET KEY HANDLING**

Never commit real secret keys to version control.
Use `.env.<network>.local` files and ensure .gitignore includes them.
See `docs/DEPLOYER_SECURITY.md` for detailed security guidance.
```

### 3. Update: `scripts/pre-flight-checklist.sh` Comments

Add security notes:
```bash
# ── Check 6: Environment Variables ────────────────────────────────────────
# SECURITY NOTE: This check only validates presence, not contents.
# Secrets are masked in output; see `mask_output()` function.
# File permissions are validated; see AC1.
```

---

## Deployment Impact

### Before Fix

- ⚠️ Secrets visible in `ps aux` output
- ⚠️ Secrets stored in shell history
- ⚠️ Secrets in CI/CD logs if debug mode enabled
- ⚠️ File permissions not validated (accidental world-readable .env)

### After Fix

- ✅ Secrets passed via env vars or stdin (not command-line args)
- ✅ History recording disabled
- ✅ Secrets masked in all output
- ✅ File permissions validated (fail if not 600)
- ✅ Explicit cleanup on exit
- ✅ Error messages don't leak secrets
- ✅ CI/CD masking guidance provided

### Operator Action Required

1. Review existing `.env` files
   - Run: `ls -la .env*`
   - Fix permissions: `chmod 600 .env*`
   - Verify none committed to git: `git log --name-only | grep .env`

2. Rotate secrets if any were ever exposed
   - Check CI logs for plain-text secrets
   - If found, regenerate deployer key on Stellar
   - Re-fund new deployer address

3. Update CI/CD configuration
   - Enable secret masking in GitHub Actions / GitLab CI
   - Example: `::add-mask::${{ secrets.STELLAR_SECRET_KEY }}`

---

## Rationale & Design Notes

### Why Layer 3 (Process Args Protection) is Critical

Stellar CLI (`stellar contract deploy`) might accept `--secret-key <arg>` directly. This is **the most dangerous** because:
- `ps aux` shows full command line to all users
- Even non-root can inspect sibling processes
- Containers share `/proc` unless isolated
- Debuggers can inspect command-line arguments

**Solution:** Use environment variables (`STELLAR_SECRET_KEY`) instead, or stdin/file descriptors.

### Why .env File Permissions Matter

A `.env` file with 644 permissions (rw-r--r--) means:
- Owner can read/write
- Group can read
- **Everyone can read** ← CRITICAL RISK

On shared systems, any user can: `cat .env` and extract all secrets.

### Why History Needs Explicit Disabling

Shells record executed commands. If a developer runs:
```bash
bash scripts/deploy.sh --env .env.mainnet
```

Even though secrets are in the .env file (not visible on command line), the script might internally echo them or use them in ways captured by history.

**Solution:** Explicitly disable history for sensitive scripts.

### Why Cleanup Handler is Important

Even if the script doesn't accidentally leak secrets, the shell environment persists. After the script exits:
```bash
# Environment still contains:
$SOURCE_SECRET
$DEPLOYER_ADDRESS
```

A child shell or subprocess can access these. Explicit `unset` ensures cleanup.

---

## FAQ

**Q: Why not just use `pass` or a secrets manager?**  
A: This audit focuses on hardening the scripts themselves. A secrets manager is complementary but not a replacement. The scripts must still handle secrets safely once retrieved.

**Q: Can I use a different CI/CD tool?**  
A: Yes, the principles apply to all CI systems. Examples provided for GitHub Actions and GitLab CI as templates.

**Q: What if I'm running on a trusted network?**  
A: Trust is not a security boundary. Even on "trusted" networks, container escapes, compromised nodes, and logging systems can leak secrets.

**Q: Does this require code changes to the contract?**  
A: No. This is a shell script hardening audit. The contract code itself doesn't change.

**Q: What about production deployments?**  
A: These fixes are **especially important** for mainnet. Use a multisig custodian or HSM (Hardware Security Module) for mainnet keys, not shell environment variables.

---

## References

- **OWASP:** Secrets Management https://owasp.org/www-community/Sensitive_Data_Exposure
- **CWE-798:** Use of Hard-Coded Credentials
- **CWE-327:** Use of Broken Cryptographic Algorithm (logging)
- **CWE-532:** Insertion of Sensitive Information into Log File
- **Soroban CLI Docs:** https://developers.stellar.org/docs/tools/soroban-cli
- **Stellar Best Practices:** https://developers.stellar.org/docs/build/guides

---

## Commit Message Template

```
[SECURITY] Harden pre-flight-checklist.sh for secret exposure prevention

- Add file permission validation for .env files (must be 600)
- Implement secret masking in all output functions
- Disable shell history recording during script execution
- Update error handlers to not dump environment state
- Refactor deploy.sh to pass secrets via env vars, not CLI args
- Add cleanup handlers to explicitly unset secrets on exit
- Create docs/DEPLOYER_SECURITY.md with comprehensive guide
- Add pre-commit hook to detect secret key patterns
- Add tests: permission checking, masking verification, history isolation

Addresses critical risk of secret exposure via:
- Process argument inspection (ps aux)
- Shell history files
- CI/CD log retention
- Error messages and stack traces
- Accidental version control commits

Tests: All passing; manual verification of ps/history protection done
Effort: 3-4 hours
```

---

**Status:** Ready for backlog  
**Owner:** Security & Deployment Team  
**Effort:** 3-4 hours (implementation + tests + documentation)
