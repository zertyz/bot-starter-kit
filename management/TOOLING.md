# Management Tooling

The management helpers live in `scripts/management/`. They enforce structure and prepare work context; they do not replace human ownership of requirements, backlog state, or acceptance.


## Validate Management Files

```bash
scripts/ci/enforce_management_rules
```

This runs the management linter and the management-tool test suite.


## Advance Backlog State

```bash
scripts/management/advance_state ER.MCP.02.a-001 --engineer Luiz
scripts/management/advance_state ER.MCP.02.a-001 --to "In Code Review" --date 2026-07-08
```

The command moves a work item to the next state by default, appends the dated state trail, and prints before/after context. Moving `Planned` to `Started` requires an engineer when the backlog item has no owner.


## Start Engineering Work

```bash
scripts/management/start_work Luiz/ER.MCP.02.a-001 --dry-run
scripts/management/start_work Luiz/ER.MCP.02.a-001
```

The command verifies that the branch references a `Started` work item and that the branch owner matches the backlog owner. New ordinary branches start from `origin/main` unless `--from-feature` or `--base` is used.


## Discuss Current Work

```bash
scripts/management/chat_about Luiz/ER.MCP.02.a-001
```

The command prints the work item, the governing requirement, and a short prompt for discussing implementation with an AI assistant.


## Evaluate a Plan

```bash
scripts/management/evaluate_plan ER.MCP.02.a-001
scripts/management/evaluate_plan Luiz/ER.MCP.02.a-001 --user Luiz
```

The command checks whether a proposed work item or branch is structurally coherent with the governing requirement. It reports requirement linkage, branch-owner fit, state readiness, work-item detail, motivation fit, and sibling work for the same requirement. It does not approve the plan.


## Draft a Plan

```bash
scripts/management/draft_plan E.MCP.02.b
scripts/management/draft_plan E.MCP.02.b --write
```

The command proposes the next syntactically valid backlog entry for a requirement and prints the existing work already tied to that requirement. By default it is a dry run; `--write` appends the entry under `Under Planning`. Use `--title`, `--motivation`, `--body-line`, `--date`, and `--allow-existing-open` when the default draft is not the intended plan.


## Audit Requirements

```bash
scripts/management/audit_requirements
scripts/management/audit_requirements --document E
scripts/management/audit_requirements --strict
```

The command reports local heuristic findings across requirements, backlog links, and `TRACEABILITY.md`: missing or thin requirement text, unbounded wording, implementation-heavy wording, unclear actors, likely compound scope, missing work, and missing traceability. It is an audit aid; market fit, competitor analysis, code-semantic drift, and final wording remain human PM/manager decisions.


## Estimate a Requirement

```bash
scripts/management/estimate_requirement E.MCP.02.b
```

The command prints a planning packet for one requirement: local readiness signals, existing work, traceability links, dependency signals, and a senior-engineer-hour effort band. The estimate is based on repo-local structure and wording; product value, market timing, and final commitment remain PM/manager decisions.


## Sync a Requirement

```bash
scripts/management/sync_requirement E.MCP.01
scripts/management/sync_requirement E.MCP.01 --strict
```

The command checks one requirement against mapped backlog work, `TRACEABILITY.md`, evidence paths, and source mentions. It reports drift signals and a local coherence verdict; it does not prove runtime behavior satisfies the requirement.


## Chase Technical Debts

```bash
scripts/management/chase_techdebts
scripts/management/chase_techdebts --limit 20
```

The command scans tracked text files for technical-debt review leads: debt markers, large files, risky Rust shortcuts in `src/`, repeated clone pressure, and sensitive terms in tracked text. Accepted findings should become Engineering or Operations requirements/backlog items with impact, deadline, and release-blocking classification.


## Check Verification Evidence

```bash
scripts/management/verification_check Luiz/ER.MCP.02.a-001
scripts/management/verification_check Luiz/ER.MCP.02.a-001 --base origin/main
```

The command inventories changed files and likely verification evidence for functional behavior, performance, security, operations, maintainability, disaster recovery, and product behavior. It reports evidence gaps; it does not decide acceptance.


## Prepare a Review Packet

```bash
scripts/management/review Luiz/ER.MCP.02.a-001
scripts/management/review Luiz/ER.MCP.02.a-001 --base origin/main
```

The command prints the governing requirement, backlog item, changed files, diff statistics, evidence summary, rule references, and a review checklist. It prepares review context; it does not approve the change.
