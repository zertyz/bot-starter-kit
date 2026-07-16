# AI Management Operating Map

This file is the fast path for agents revisiting the project-management system. It explains where authority lives and how to act without rediscovering the governance model.


## Read Order

1. `management/MANAGEMENT.md` for the governing process.
2. `management/DEFINITION_OF_READY_DONE.md` for state-transition rules.
3. `management/TOOLING.md` for the command surface.
4. The relevant requirement file: `BUSINESS.md`, `ENGINEERING.md`, or `OPERATIONS.md`.
5. The matching backlog file.
6. `management/TRACEABILITY.md`, then ledgers that apply: `DECISIONS.md`, `RISKS.md`, `INCIDENTS.md`, `EXPERIMENTS.md`, `RELEASES.md`.


## Source of Truth

Requirements describe desired behavior. Backlogs describe planned work. Code in `main` describes current behavior. Tests, benchmarks, reviews, operations notes, releases, and linked files are evidence.

When those disagree, treat it as requirement drift. Report the drift and use the repo-local tools to gather context. Do not silently rewrite requirements, backlog history, comments, or evidence to make the disagreement disappear.


## Generated and Interactive Surfaces

GitHub Pages is read-only. It publishes the static Management Report, diagrams, Rust docs, coverage, and benchmarks. It cannot run the local GUI, edit files, commit, tag, or push.

The static Management Report is an offline export of the GUI shell:

```bash
scripts/management/management_report --html public/management
```

It writes `index.html`, `model.json`, frontend assets, and generated diagrams. Refresh and action buttons remain visible there, but are disabled with a local-online-mode tooltip.

The local GUI is an authoring console over allowlisted management commands:

```bash
scripts/management/gui
```

Use it for interactive planning and review. Use the CLI directly for scripted work, tests, CI, and precise terminal evidence. The local GUI and static report share `scripts/management/gui_server` plus `scripts/management/gui_static/`; keep new views and model fields shared unless a feature genuinely requires a localhost-only action path.


## Normal Agent Workflow

Before proposing or editing management work:

1. Run or inspect `scripts/management/management_tool lint`.
2. Identify the governing requirement and matching backlog item.
3. Check whether traceability and ledgers already answer the question.
4. For qualitative analysis, run the matching `semantic_context` workflow and use `$analyze-management`; inspect the cited code, tests, and git history rather than treating the packet as a conclusion.
5. Prefer dry runs before writes: `draft_plan`, `advance_state`, `block_work`, `unblock_work`, release tagging, and branch commands all have dry-run paths.
6. Keep generated dashboard and diagram outputs out of commits unless they are intentionally published artifacts.

After changing management tooling or management files:

1. Run `scripts/ci/enforce_management_rules`.
2. Run `scripts/ci/enforce_code_guidelines` when project scripts or Rust source changed.
3. Run `git diff --check`.
4. If Python imports created `__pycache__`, remove it before finishing.


## Command Intent

- `management_status`: summarize the current management state; its compact HTML dashboard mode is compatibility-only.
- `management_report`: export the static GUI-based Management Report for GitHub Pages or local review.
- `diagram_architecture_real`, `diagram_architecture_planned`, `diagram_module_dependencies`: render static SVG diagrams for Pages or local review.
- `semantic_context`: provide versioned deterministic evidence for the repo-local `$analyze-management` semantic workflow.
- `audit_requirements`: find local requirement quality and planning issues.
- `estimate_requirement`: prepare PM/manager planning context for one requirement.
- `sync_requirement` and `trace_requirement`: compare one requirement against backlog, traceability, and implementation signals.
- `draft_plan`: propose or write an `Under Planning` backlog item.
- `evaluate_plan`: check whether a proposed work item coheres with its requirement.
- `advance_state`: move a backlog item through the state model.
- `block_work` and `unblock_work`: append blocker history without changing lifecycle state.
- `start_work`, `chat_about`, `verification_check`, `review`, `acceptance_packet`: prepare engineering execution, review, and acceptance context.
- Ledger commands record and close decisions, risks, incidents, and experiments.


## Write Discipline

Preserve intent. Do not delete code, comments, strings, requirements, backlog entries, or ledger history just because they look wrong. If a record is stale or contradictory, add the right follow-up record or ask the human owner.

Write the smallest coherent change that keeps the governance graph understandable: requirement, work item, evidence, decision/risk/incident/experiment, and tooling should point to each other when they are related.
