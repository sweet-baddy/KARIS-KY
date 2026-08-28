# Issue Summary: Secrets Exposure in Deployment Scripts

**File:** `SECURITY_ISSUE_ENV_SECRET_EXPOSURE.md`  
**Type:** Security / Secrets management  
**Priority:** CRITICAL  
**Severity:** CRITICAL  
**Status:** Backlog (design audit)

---

## Quick Reference

| Field | Value |
|-------|-------|
| **Issue** | Audit pre-flight-checklist.sh for secret exposure in environment variables |
| **Severity** | CRITICAL (secrets can compromise mainnet deployments) |
| **Status** | Backlog |
| **Component** | `scripts/pre-flight-checklist.sh`, `scripts/deploy.sh`, related scripts |
| **Affected Versions** | All (current scripts) |
| **Requires Compromise** | No - risk is built-in to design |
| **Blocked By** | Nothing |
| **Blocks** | Mainnet deployment (should not deploy without fixes) |

---

## Problem Summary

Deployment scripts handle **Stellar private keys** (secrets starting with 'S', 56 characters) via environment variables with **insufficient protections**. Secrets can leak through:

- ❌ **Process inspection** (`ps aux` shows full command line with secret keys)
- ❌ **Shell history** (`.bash_history` contains commands with secrets)
- ❌ **CI/CD logs** (GitHub Actions/GitLab logs show full output including secrets)
- ❌ **Error messages** (deployment failures output command line with secrets)
- ❌ **File permissions** (.env files not validated for world/group readability)
- ❌ **Version control** (.env files committed to git with placeholder guidance)

### Current Risk Level

**CRITICAL** — Mainnet keys could be compromised if:
1. Developer runs deployment manually on shared system
2. CI/CD system is compromised and logs are retained
3. .env file accidentally commits with real secrets
4. Error occurs during deployment and exception shows full command

---

## Threat Model

| Threat | Vector | Impact |
|--------|--------|--------|
| **Shared server admin** | Can inspect running `ps` output | Stellar key exposed |
| **Container escape** | Read environment/process memory | Keys extracted |
| **CI/CD attacker** | Reads old build logs | Secrets in plaintext |
| **Git history scan** | Searches repo for S-prefix patterns | Secrets recoverable |
| **Malicious script** | Inherits parent environment | Keys exfiltrated |
| **Developer mistake** | Screenshots shell session | Secrets in screenshot |

---

## What's Missing (Gap Analysis)

| Protection | Today | Needed |
|-----------|-------|--------|
| **File permission check** | ❌ No validation | ✅ Validate .env is 600 |
| **Secret masking in output** | ❌ Plaintext everywhere | ✅ Mask all S-prefixed keys |
| **History protection** | ❌ Secrets recorded | ✅ Disable HISTFILE or use HISTCONTROL |
| **CLI arg protection** | ❌ `--secret-key $S...` visible in ps | ✅ Use env vars or stdin |
| **Cleanup on exit** | ❌ Env persists after script | ✅ Explicit unset/shred |
| **Error handling** | ❌ Full command in error msg | ✅ Redacted generic messages |
| **CI/CD masking** | ❌ Logs show secrets | ✅ Document masking setup |
| **Version control** | ⚠️ Placeholders OK but weak guidance | ✅ .env.local pattern + pre-commit hook |

---

## Solution Overview

Implement **3-layer hardening**:

### Layer 1: Input Protection
- Validate .env file is not world/group readable (chmod 600 required)
- Use safer file sourcing patterns
- Explicit variable extraction

### Layer 2: Processing Protection
- Pass secrets via environment variables (not CLI args)
- Implement secret masking function for all output
- Strict error handling (no env dump)

### Layer 3: Output Protection
- Redact all secrets from logs and error messages
- Disable shell history recording
- Explicit cleanup/unset on exit

---

## Acceptance Criteria

✅ **10 detailed criteria** defined in full specification:

1. Input validation (file permissions)
2. Secret masking in output
3. History protection
4. Process argument protection
5. Error handler hardening
6. CI/CD integration guide
7. .env file security guide
8. Testing & verification
9. Documentation updates
10. Backward compatibility

---

## Implementation

| Aspect | Detail |
|--------|--------|
| **Effort** | 3-4 hours |
| **Code changes** | ~100-150 lines (masking, error handling, cleanup) |
| **Files modified** | 3-4 scripts + 2-3 docs |
| **Tests** | 3 unit tests + 2 manual tests |
| **Breaking change** | NO (backward compatible) |
| **Requires redeploy** | NO (script changes only) |

---

## Key Changes Required

### In `pre-flight-checklist.sh`

```bash
# 1. Validate .env permissions
if [[ "$FILE_PERMS" == *"4"* ]] || [[ "$FILE_PERMS" == *"2"* ]]; then
  fail ".env file is world/group readable"
fi

# 2. Add masking function
redact() {
  echo "${1:0:2}...${1: -2}"
}

# 3. Disable history
export HISTCONTROL=ignorespace
set +x

# 4. Cleanup on exit
trap 'unset SOURCE_SECRET' EXIT
```

### In `deploy.sh`

```bash
# Current (bad):
stellar contract deploy --secret-key "${SOURCE_SECRET}" ...

# Fixed (good):
export STELLAR_SECRET_KEY="${SOURCE_SECRET}"
stellar contract deploy ...  # reads from env, not CLI args
```

---

## Testing Strategy

| Test | Purpose |
|------|---------|
| `test_env_file_permissions_checked` | Verify 644 permissions fail, 600 pass |
| `test_secrets_masked_in_output` | Verify S-keys masked as S...MASKED... |
| `test_history_disabled` | Verify history file empty after execution |
| **Manual:** `ps aux during deploy` | Verify no plain secret in process listing |
| **Manual:** `CI log inspection` | Verify logs don't show S-prefixed keys |

---

## Deployment Impact

### Before Fix
- ⚠️ Secrets visible in `ps aux` (CRITICAL)
- ⚠️ Secrets in shell history (CRITICAL)
- ⚠️ Secrets in CI logs if debug enabled (CRITICAL)
- ⚠️ File permissions not checked (CRITICAL)

### After Fix
- ✅ Secrets passed via env vars (not CLI args)
- ✅ History disabled during execution
- ✅ Secrets masked in all output
- ✅ File permissions validated
- ✅ Cleanup on exit
- ✅ Error messages redacted

### Operator Action
1. Review/fix `.env*` file permissions (chmod 600)
2. Enable CI/CD secret masking
3. Rotate Stellar keys if any were exposed
4. Review git history for leaked secrets

---

## Security Improvements

| Metric | Before | After | Improvement |
|--------|--------|-------|------------|
| **Secret visibility in `ps`** | HIGH | NONE | 100% eliminated |
| **History leakage** | HIGH | NONE | 100% eliminated |
| **CI log leakage** | HIGH | MASKED | ~99% reduction |
| **File permission risk** | HIGH | CHECKED | 100% fail-fast |
| **Error message leakage** | HIGH | REDACTED | 100% eliminated |

---

## Error Code (Reference)

Not needed for shell scripts, but documented:
- Error code for mainnet deployment rejection: N/A (operational check)
- Pre-commit hook detection: Pattern-based (S[A-Z0-9]{55})

---

## Files Modified (When Implemented)

- `scripts/pre-flight-checklist.sh` (add checks & masking)
- `scripts/deploy.sh` (update arg passing)
- `scripts/verify_deployment.sh` (add masking)
- `docs/DEPLOYER_SECURITY.md` (new file)
- `.pre-commit-hooks.yaml` (new file)

---

## Related Issues

- **Migrate replay protection:** Similar defense-in-depth (input → processing → output)
- **Attestation rate-limiting:** Similar layered approach
- **General secrets management:** OWASP best practices

---

## FAQ

**Q: Why is this CRITICAL?**  
A: Mainnet deployment requires Stellar secret keys. Exposure compromises the entire escrow deployment. This is not a code bug but a design flaw in secret handling.

**Q: Can I use an HSM (Hardware Security Module)?**  
A: Yes, recommended for mainnet. These scripts still need hardening for testnet and staging.

**Q: What if the CI/CD system is compromised?**  
A: These fixes reduce the attack surface. Masking + history disabling prevent opportunistic leaks. For mainnet, use dedicated CI with restricted access.

**Q: Do I need to change the contract code?**  
A: No. This is a shell script hardening audit only. Contract code unchanged.

**Q: What about .env files in version control?**  
A: Placeholders are safe (they're not real keys). Guidance is weak and needs .env.local + pre-commit hook.

---

## Priority & Timeline

- **Deploy before:** Mainnet production deployment
- **Must have:** File permission validation, secret masking
- **Nice to have:** Pre-commit hook, detailed audit guide
- **Consider:** HSM integration (future, not this fix)

---

## Questions?

**For problem details:** Read full spec `SECURITY_ISSUE_ENV_SECRET_EXPOSURE.md`

**For implementation:** Follow "Proposed Solution" section in full spec

**For testing:** See "Testing Strategy" section

---

## References

- Full issue: `SECURITY_ISSUE_ENV_SECRET_EXPOSURE.md`
- OWASP secrets management: https://owasp.org/www-community/Sensitive_Data_Exposure
- CWE-532: Insertion of Sensitive Information into Log Files
- Pre-commit hooks: https://pre-commit.com/
