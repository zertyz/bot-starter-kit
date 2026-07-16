# Semantic Management Workflows

Use the common workflow in `SKILL.md`, then apply only the section matching the user's request.

## Audit Requirement Drift

- Compare the current requirement text and recorded decisions with the implementation, tests, configuration, and user-visible behavior.
- Check whether code implements an older interpretation, adds unrequested behavior, or omits a stated constraint.
- Distinguish deliberate divergence recorded in a decision from accidental drift.

## Estimate Work

- Identify affected components, interfaces, data migrations, tests, documentation, deployment steps, and external dependencies.
- Inspect analogous completed work and relevant history when available.
- Return assumptions, uncertainty drivers, and a range or decomposition rather than false precision.

## Evaluate a Plan

- Map each plan step to requirement outcomes and verification evidence.
- Find missing prerequisites, unsafe ordering, unowned decisions, and changes outside the authorized scope.
- Prefer the smallest domain model that supports the requirement and credible future evolution.

## Review Implementation

- Inspect the actual diff and its surrounding code, not only management records.
- Check correctness, preservation of intent, concurrency and failure behavior, security boundaries, maintainability, and matching tests.
- Lead with actionable findings ordered by severity; say explicitly when none are found.

## Verify Evidence

- Re-run the narrowest relevant tests and checks where practical.
- Confirm traceability paths exist and substantively support the claimed requirement.
- Separate passing automation from behavioral evidence that still needs human acceptance.

## Synchronize Management State

- Compare requirements, backlog state/history, blockers, traceability, decisions, releases, risks, and implementation evidence.
- Identify contradictions and stale records without silently choosing which source the human intended.
- Propose precise record updates, but make them only when the user authorizes changes.

## Analyze Technical Debt

- Tie debt to concrete maintenance cost, correctness risk, security exposure, delivery friction, or requirement evolution.
- Distinguish intentional tradeoffs and deferred policy decisions from accidental debt.
- Rank remediation by impact, likelihood, and coupling to planned work.
