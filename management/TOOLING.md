# Management Tooling

The management helpers live in `scripts/management/`. They enforce structure and prepare work context; they do not replace human ownership of requirements, backlog state, or acceptance.


## Validate Management Files

```bash
scripts/ci/enforce_management_rules
```

This runs the strict management linter, core and GUI test suites, and a static-report export check.


## Management Status

```bash
scripts/management/management_status
scripts/management/management_status --html public/management
```

The command prints a project-management status summary. Its `--html` mode remains available for the older compact dashboard format, but GitHub Pages publishes the GUI-based Management Report below.


## Static Management Report

```bash
scripts/management/management_report
scripts/management/management_report --html public/management
```

The command exports an offline static version of the management GUI: `index.html`, `model.json`, reused CSS/JS assets, and generated SVG diagrams. It reads from git-controlled management files at generation time. Refresh and action buttons stay visible in the static report, but they are disabled because Pages cannot run local commands, edit files, commit, tag, or push.


## Local Management GUI

```bash
scripts/management/gui
scripts/management/gui --no-browser
scripts/management/gui --port 8780
scripts/management/gui --host 0.0.0.0 --unsafe-allow-non-loopback --allowed-host management.example.test
```

The GUI binds to loopback by default and serves a browser console over the same management files and helper commands. It shows requirements, backlog work, ledgers, diagrams, status metrics, audit signals, tech-debt leads, and workflow forms for planning, state changes, evidence links, releases, branch checks, and ledger entries. Browser actions are allowlisted by the Python server; they do not execute arbitrary shell commands. The local session uses a per-process token and rejects invalid hosts, cross-origin actions, unexpected content types, and request bodies larger than 64 KiB.

Binding to a non-loopback address requires the explicit `--unsafe-allow-non-loopback` acknowledgement. Add each expected HTTP host name with the repeatable `--allowed-host` option; this does not add TLS or make the authoring console suitable for public exposure.

The local GUI and static Management Report share the same model builder and frontend assets. Add new management views to both surfaces by extending `scripts/management/gui_server` and `scripts/management/gui_static/` together, then verify with `scripts/management/test_gui_server`.


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
scripts/management/advance_state EN.Demo.02-001 --to "In Code Review" --date 2026-07-08
scripts/management/advance_state WORK_ITEM_ID --to "Superseded by EXISTING_WORK_ITEM_ID" --dry-run
scripts/management/advance_state WORK_ITEM_ID --to Merged --override-gate "Manual acceptance evidence is recorded in review 42"
```

The command moves a work item to the next state by default, appends the dated state trail, and prints before/after context. Moving `Planned` to `Started` requires an engineer when the backlog item has no owner. Blocked work cannot advance. `Merged` requires traceability and concrete evidence; `Rolled Out` also requires a release record and operational or release evidence. It rejects backward, skipped, backdated, and post-terminal transitions. Supersession must name an existing work item and is terminal.

Actual state changes are allowed only on `main`, as required by `MANAGEMENT.md`; `--dry-run` remains available on work branches. `--override-gate` records a reason and can bypass only missing evidence gates, never ownership, blocker, ordering, terminal-state, or date rules.

`advance_state` prints the commands that normally follow a successful transition or dry run. The complete lifecycle command map is:

| Current state | Preparation and next command |
| --- | --- |
| `Under Planning` | Run `evaluate_plan`, then dry-run `advance_state WORK_ITEM_ID --to Planned`. |
| `Planned` | Dry-run and then run `advance_state WORK_ITEM_ID --engineer ENGINEER`; after it reaches `Started`, use `start_work`. |
| `Started` | Use `start_work`, then `chat_about`; before review, run `verification_check` and `review`. |
| `In Code Review` | Run `verification_check` and `review`, then choose `QA`, `Integrated`, or direct `Merged` according to the work. |
| `Integrated` | Link concrete evidence, inspect it with `trace_requirement`, then dry-run the move to `Merged`. |
| `QA` | Use `acceptance_packet`, link concrete evidence, inspect it, then dry-run the move to `Merged`. |
| `Merged` | Use `prepare_release`, optionally create the approved tag, record the observed outcome with `record_release`, link `management/RELEASES.md`, then dry-run `Rolled Out`. |
| `Rolled Out` | The lifecycle is complete; use `trace_requirement` and `sync_requirement` for final inspection. |
| `Rejected`, `Cancelled`, or `Superseded by ...` | Terminal; no further lifecycle transition applies. |

Blocked is independent of those states. When an advance fails because the item is blocked, run `unblock_work WORK_ITEM_ID --reason "..."` after the blocker has actually been resolved.

When the command reports that a work item has no `TRACEABILITY.md` row or no existing concrete evidence path, add both with `link_evidence`, inspect the result, and retry the transition:

```bash
scripts/management/link_evidence WORK_ITEM_ID EXISTING_REPO_PATH [MORE_EXISTING_PATHS ...]
scripts/management/trace_requirement REQUIREMENT_ID
scripts/management/advance_state WORK_ITEM_ID --dry-run
scripts/management/advance_state WORK_ITEM_ID
```

For `EF.Gov.01-002`, the governing requirement is `E.Gov.01`. A concrete command covering the management implementation, tests, documentation, and bug-report intake is:

```bash
scripts/management/link_evidence EF.Gov.01-002 \
  --note "Management command guidance, bug-report intake, and shared GUI/report support" \
  scripts/management/management_tool \
  scripts/management/gui_server \
  scripts/management/gui_static/app.js \
  scripts/management/gui_static/index.html \
  scripts/management/test_management_tool \
  scripts/management/test_gui_server \
  management/BUGS.md \
  management/TOOLING.md
```

Link only artifacts that substantively support the work. At least one linked path must already exist for the deterministic evidence gate to pass; a note alone is not a concrete path, and `--allow-missing` does not make a missing path satisfy the gate.

When the target is `Rolled Out`, evidence of implementation alone is insufficient. The work item must be named in a schema-valid `Released` entry and must link release or operational evidence. The release workflow below creates that record without manually editing `RELEASES.md`.


## Block and Unblock Work

```bash
scripts/management/block_work ER.MCP.02.a-001 --reason "Waiting for contract decision" --dry-run
scripts/management/block_work ER.MCP.02.a-001 --reason "Waiting for contract decision"
scripts/management/unblock_work ER.MCP.02.a-001 --reason "The contract decision is recorded"
```

Blocking is append-only history independent of lifecycle state. Repeated block or unblock operations that do not change the current blocked condition are rejected. Actual writes require `main`; `--dry-run` can be used elsewhere.

Use `block_work` when a specific work item temporarily cannot proceed; it does not move that item's lifecycle state. Before writing, it validates the complete management graph. An error naming another management file means that record must be made coherent before the blocker can be appended.


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


## Build Semantic Analysis Context

```bash
scripts/management/semantic_context audit E.Gov.01
scripts/management/semantic_context plan EN.Gov.01-001
scripts/management/semantic_context estimate E.Gov.01
scripts/management/semantic_context sync E.Gov.01
scripts/management/semantic_context review Codex/EN.Gov.01-001 --base origin/main
scripts/management/semantic_context verification EN.Gov.01-001
scripts/management/semantic_context tech-debt --limit 20
```

The command emits versioned JSON that keeps deterministic quality findings, lexical semantic prompts, `work_coverage`, and lifecycle-aware `evidence_coverage` separate. Prompts are `active` or `acknowledged`; they are review leads, never semantic verdicts. Pass that context to the repo-local `$analyze-management` skill when the task requires judgment about requirements, plans, implementation quality, verification, drift, or technical debt.

An optional requirement on `audit` includes that parent requirement and its descendants. `estimate` and `sync` expose descendants alongside their exact focus. An optional repository path on `tech-debt` narrows candidates to that path prefix.

The skill lives in `.agents/skills/analyze-management/`. It reads the governing records, consumes the evidence packet, then inspects the actual implementation, tests, and relevant git history. Its report separates observed evidence, engineering inference, and unknowns; the human owner still controls requirement changes and final acceptance.


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
scripts/management/draft_plan E.Gov.01 --motivation F --title "Fix management command behavior" --write
```

The command proposes the next syntactically valid backlog entry for a requirement and prints the existing work already tied to that requirement. By default it is a dry run; `--write` appends the entry under `Under Planning`. Use `--title`, `--motivation`, `--body-line`, `--date`, and `--allow-existing-open` when the default draft is not the intended plan.

`BUGS.md` is the intake ledger for unresolved reports. A report may be unverified, may be a misunderstanding, and need not initially identify a Business, Engineering, or Operations requirement. It is therefore not planned work. Only entries under the single exact `## Open Reports` section are validated and included in GUI, status, dashboard, and meeting-packet counts; other level-two sections are inactive history.

After validation, create an `F`-motivation work item for any required corrective work and link it from the report. Keep the report current until that work reaches `Rolled Out`. If investigation resolves the report through clarification or documentation without corrective work, keep it current until that disposition is complete. An addressed report may then move to `## Addressed Reports` while useful and be deleted later; git history also retains it. `INCIDENTS.md` remains reserved for production, staging, data, security, or release failures.


## Audit Requirements

```bash
scripts/management/audit_requirements
scripts/management/audit_requirements --document E
scripts/management/audit_requirements --strict
```

The command reports four independent dimensions: deterministic quality findings; active and acknowledged semantic prompts; work coverage; and lifecycle-aware evidence coverage. Exact single-emphasis around a broad term, such as `*all*`, acknowledges that term for the whole requirement without erasing the observation. `--strict` fails only on deterministic `BLOCKER` or `REVIEW` quality findings; unplanned work, pending evidence, and lexical prompts do not make requirement quality fail. Market fit, code-semantic drift, and final wording remain human PM/manager decisions.


## Estimate a Requirement

```bash
scripts/management/estimate_requirement E.MCP.02.b
```

The command prints a planning packet for one requirement: deterministic readiness findings, semantic prompts, work coverage, parsed traceability links, evidence coverage, dependency signals, and a structural sizing signal. Active prompts require semantic review but do not become quality findings; acknowledged boundedness or scope remains an author decision. The deterministic command always withholds a finite delivery estimate: lexical silence cannot establish scope, dependencies, implementation context, or effort, and the structural heuristic is never relabeled as a commitment. Product value, market timing, semantic disposition, and final estimation remain PM/manager decisions.


## Sync a Requirement

```bash
scripts/management/sync_requirement E.MCP.01
scripts/management/sync_requirement E.MCP.01 --strict
```

The command checks one requirement against mapped backlog work, `TRACEABILITY.md`, evidence paths, and source mentions. It reports drift signals and a local coherence verdict; it does not prove runtime behavior satisfies the requirement.


## Trace and Link Evidence

```bash
scripts/management/trace_requirement E.MCP.01
scripts/management/link_evidence EN.MCP.01-001 src/messaging/user_router.rs
scripts/management/link_evidence EN.MCP.01-001 src/messaging/user_router.rs --note "Routing implementation and same-module tests"
```

Evidence is a concrete artifact that supports a requirement or work item: code, tests, benchmarks, review notes, release notes, operational logs, incident follow-up, or documentation. Paths are repository-relative and normally must exist. `trace_requirement` shows the parsed Current Links row and reports whether its evidence paths exist. `link_evidence` derives the exact governing requirement from the work item, validates the paths, then creates or updates the row using the work item's current state. Repeating it appends evidence that is not already present. The older `REQUIREMENT_ID WORK_ITEM_ID` form remains accepted for compatibility, but the requirement argument is unnecessary.

Use `--note` for a short explanation in addition to concrete paths. Use `--allow-missing` only to record a planned artifact; a missing artifact remains insufficient for a `Merged` or `Rolled Out` evidence gate.


## Release Commands

```bash
scripts/management/prepare_release 1.2.3-rc.1
scripts/management/create_release_tag 1.2.3-rc.1
scripts/management/create_release_tag 1.2.3-rc.1 --execute
scripts/management/record_release 1.2.3 EF.Gov.01-002 \
  --decision Released \
  --verification "Branch, main, post-merge, and operational checks passed" \
  --rollback "Restore the preceding deployed revision" \
  --notes "Management tooling guidance release"
scripts/management/link_evidence EF.Gov.01-002 management/RELEASES.md
scripts/management/advance_state EF.Gov.01-002 --dry-run
scripts/management/advance_state EF.Gov.01-002
```

`prepare_release` prints a packet containing the currently `Merged` work and never creates tags. `create_release_tag` validates the tag format and clean worktree; it is a dry run unless `--execute` is passed, and it creates only a local tag. Pushing the tag and observing deployment checks remain deliberate release-owner actions.

After the outcome is known, `record_release` writes one structured entry before the retained format section in `RELEASES.md`. It derives requirement IDs and summaries from the supplied work item IDs, accepts only `Merged` or already `Rolled Out` work, and requires an explicit decision, verification, rollback or mitigation, and notes. It does not create or push a tag, deploy, link evidence, or change backlog states. A release-candidate version cannot have a production decision of `Released`. For a production `Released` decision, link `management/RELEASES.md` to each still-`Merged` work item before retrying `advance_state`. A `Rejected` or `Rolled back` entry documents the outcome but cannot satisfy the `Rolled Out` gate.


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
