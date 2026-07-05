# Risks

Risks are active until closed. Each risk should point to requirements, work items, decisions, or releases when those links exist.


## Open Risks

### RISK-0001 -- Messaging Platform Feature Drift

Status: Open
Owner: Product Manager
Related: `B.MsgP.02.a`

Risk:
Messaging platforms may add features faster than the project updates its common contracts and gateways.

Impact:
The bot may lag behind platform capabilities or require platform-specific workarounds.

Mitigation:
Review supported platform features at least every three months and create backlog work for relevant gaps.


### RISK-0002 -- Formally Persisted Data Is Not Yet Fully Governed

Status: Open
Owner: Engineering Manager
Related: `O.DS.02`

Risk:
Data that users or revenue depend on may be persisted before the inspection and manual-change requirements are fully implemented.

Impact:
Operational recovery and support work may become harder than expected.

Mitigation:
Do not treat formally persisted data as production-ready until inspectability and controlled mutation are implemented and verified.


### RISK-0003 -- Starter-Kit Scope Can Outrun Its Architecture

Status: Open
Owner: Engineering Manager
Related: `E.Arch.01`, `E.Arch.02`, `E.Arch.03`

Risk:
New examples, storage wrappers, and platform integrations may add behavior faster than the responsibility layers mature.

Impact:
The project may become a collection of demos instead of a coherent starter kit.

Mitigation:
Require new foundational changes to reference requirements, decisions, and traceability before merge.


### RISK-0004 -- Webhook Exposure and Secret Handling

Status: Open
Owner: Operations
Related: `scripts/operations/create_telegram_webhook_tls`, `scripts/operations/gcp_webhook_firewall`

Risk:
Webhook endpoints, certificates, private keys, and firewall rules can expose production infrastructure when configured incorrectly.

Impact:
Unauthorized traffic, failed webhook delivery, or leaked secrets.

Mitigation:
Keep operations helpers explicit, fail-fast, and documented. Record production-facing changes in releases and incidents when they affect availability or security.


## Closed Risks

No risks closed yet.
