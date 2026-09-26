# Deployer Security Guide

> **Key Terms:** [funding close snapshot](docs/glossary.md#funding-close-snapshot) · [pro-rata denominator](docs/glossary.md#pro-rata-denominator) · [SME](docs/glossary.md#sme)

This guide covers the security considerations and operational practices for deploying and managing the escrow contracts.

## Overview

Deployers are responsible for the secure configuration, deployment, and ongoing administration of the escrow system. This document outlines the key security practices, threat model, and operational procedures.

## Roles and Responsibilities

### Deployer

The deployer is the account that deploys the escrow contracts and configures the initial parameters. The deployer typically holds administrative privileges during the setup phase.

### Administrator

The administrator manages ongoing operations, including parameter updates, attestation management, and compliance actions.

## Deployment Checklist

Before deploying to production, ensure the following:

1. **Key Management**: Use a hardware wallet or secure key management solution for the deployer account.
2. **Parameter Review**: Verify all initialization parameters against the [escrow-init-parameters.md](escrow-init-parameters.md) documentation.
3. **Fund Parameters**: Confirm fund parameters match the [escrow-fund-parameters.md](escrow-fund-parameters.md) specification.
4. **Attestations**: Review the [escrow-attestations.md](escrow-attestations.md) documentation for attestation requirements.
5. **Compliance**: Ensure compliance requirements are met per the [escrow-compliance-guide.md](escrow-compliance-guide.md).

## Key Management

### Deployer Keys

- Store deployer private keys in a hardware wallet or secure enclave.
- Never commit private keys or mnemonics to version control.
- Use separate accounts for deployment and ongoing administration where possible.
- Rotate keys according to your organization's security policy.

### Administrator Keys

- Administrators should use multi-signature wallets for critical operations.
- Document all administrator key holders and their responsibilities.
- Establish clear procedures for key rotation and emergency access.

## Threat Model

### Compromised Deployer Key

If the deployer key is compromised, an attacker could:

- Reconfigure contract parameters.
- Initiate unauthorized fund movements.
- Modify attestation requirements.

**Mitigation**: Use hardware wallets, multi-signature schemes, and monitor for unexpected transactions.

### Malicious Administrator

A malicious administrator could abuse privileged functions to:

- Freeze or unfreeze funds inappropriately.
- Modify compliance settings.
- Approve fraudulent attestations.

**Mitigation**: Implement multi-signature requirements, establish audit trails, and separate duties across multiple administrators.

### Front-Running

Attackers may attempt to front-run deployment or configuration transactions.

**Mitigation**: Use private transaction pools where available, and monitor mempool activity during critical operations.

## Operational Security

### Monitoring

- Monitor all administrative transactions and alert on unexpected activity.
- Track contract state changes, especially around the [funding close snapshot](docs/glossary.md#funding-close-snapshot).
- Set up alerts for parameter changes and attestation updates.

### Incident Response

1. **Detect**: Identify the incident through monitoring or reports.
2. **Contain**: Pause operations if possible and secure remaining funds.
3. **Assess**: Determine the scope and impact of the incident.
4. **Remediate**: Apply fixes and restore normal operations.
5. **Review**: Conduct a post-mortem and update procedures.

### Access Control

- Follow the principle of least privilege.
- Regularly review and revoke unnecessary permissions.
- Maintain an up-to-date inventory of all privileged accounts.

## Compliance Considerations

Deployers must ensure the system is configured to meet applicable regulatory requirements. See the [escrow-compliance-guide.md](escrow-compliance-guide.md) for detailed compliance procedures.

### Legal Holds

Understand how legal holds interact with fund operations. See [escrow-legal-hold.md](escrow-legal-hold.md) for details.

## State Machine

The escrow contracts follow a defined state machine. Review [state-machine.md](state-machine.md) to understand valid state transitions and their security implications.

## Audit and Verification

- Conduct regular security audits of the deployed contracts.
- Verify contract source code matches deployed bytecode.
- Maintain records of all audits and their findings.
- Address audit findings promptly and document remediation.

## Additional Resources

- [OPERATOR_RUNBOOK.md](OPERATOR_RUNBOOK.md) — Operational procedures
- [investor-quick-start.md](investor-quick-start.md) — Investor onboarding
- [docs/glossary.md](docs/glossary.md) — Key terms and definitions
