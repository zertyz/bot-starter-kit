# Management Tooling

The management helpers live in `scripts/management/`. They enforce structure and prepare work context; they do not replace human ownership of requirements, backlog state, or acceptance.


## Validate Management Files

```bash
scripts/ci/enforce_management_rules
```

This runs the management linter and the management-tool test suite.


## Management Dashboard

```bash
scripts/management/management_status
scripts/management/management_status --html public/management
```

The command prints a project-management status summary and can render a static HTML dashboard. GitHub Pages publishes the dashboard next to Rust docs, coverage, and benchmark reports.


## Local Management GUI

```bash
scripts/management/gui
scripts/management/gui --no-browser
scripts/management/gui --port 8780
```

The GUI runs a localhost-only browser console over the same management files and helper commands. It shows requirements, backlog work, ledgers, diagrams, status metrics, and workflow forms for planning, state changes, evidence links, releases, branch checks, and ledger entries. Browser actions are allowlisted by the Python server; they do not execute arbitrary shell commands.


## Architecture Diagrams

```bash
scripts/management/diagram_architecture_real
scripts/management/diagram_architecture_planned
scripts/management/diagram_module_dependencies
scripts/management/diagram_module_dependencies --scope messaging
```

The diagram commands render static SVG files under `public/management/diagrams` by default. `diagram_architecture_real` derives the current architecture from Rust source dependencies. `diagram_architecture_planned` overlays open backlog work in blue and requirements with no mapped work in orange. `diagram_module_dependencies` shows Rust module-to-module references; `--scope` narrows the graph while keeping directly connected modules visible.


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


## Trace and Link Evidence

```bash
scripts/management/trace_requirement E.MCP.01
scripts/management/link_evidence E.MCP.01 EN.MCP.01-001 src/messaging/user_router.rs
```

Evidence is a concrete artifact that supports a requirement or work item: code, tests, benchmarks, review notes, release notes, operational logs, incident follow-up, or documentation. `trace_requirement` shows the current knowledge base for one requirement. `link_evidence` updates `TRACEABILITY.md` after validating the requirement, work item, and evidence paths.


## Release Commands

```bash
scripts/management/prepare_release 1.2.3-rc.1
scripts/management/create_release_tag 1.2.3-rc.1
scripts/management/create_release_tag 1.2.3-rc.1 --execute
```

`prepare_release` prints a release decision packet and never creates tags. `create_release_tag` validates the tag format and clean worktree; it is a dry run unless `--execute` is passed.


## Supporting Ledgers

```bash
scripts/management/record_decision "Use gateway-owned routing" --context "..." --decision "..." --consequence "..."
scripts/management/record_risk "Feature drift" --owner "Product Manager" --risk "..." --impact "..." --mitigation "..."
scripts/management/close_risk RISK-0001 --evidence "..."
scripts/management/record_incident "Webhook outage" --environment Staging --impact "..." --timeline "..." --mitigation "..."
scripts/management/close_incident INCIDENT-0001 --evidence "..."
scripts/management/record_experiment "Try platform feature cache" --branch Luiz/EX.MCP.02.b-001 --hypothesis "..." --expires 2026-08-01 --success "..." --failure "..."
scripts/management/close_experiment EXP-0001 --status Adopted --result "..."
```

A decision is an accepted project choice that constrains future work; it should record context, the decision, consequences, and related requirements or work. A risk is a known possible future problem with owner, impact, mitigation, and review date. An incident is a production, staging, data, security, or release failure that teaches the project something operationally useful. An experiment is a time-boxed `X` branch, spike, prototype, or research effort with hypothesis, expiration, criteria, and result.


## Meeting and Acceptance Packets

```bash
scripts/management/stale_work
scripts/management/meeting_packet
scripts/management/acceptance_packet EN.MCP.01-001
```

`stale_work` lists active work older than the configured threshold. `meeting_packet` prints a manager/PM/engineering sync packet. `acceptance_packet` gathers requirement, definition-of-done, traceability, and verification context for human acceptance.


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
