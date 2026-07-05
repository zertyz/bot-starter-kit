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


## Check Verification Evidence

```bash
scripts/management/verification_check Luiz/ER.MCP.02.a-001
scripts/management/verification_check Luiz/ER.MCP.02.a-001 --base origin/main
```

The command inventories changed files and likely verification evidence for functional behavior, performance, security, operations, maintainability, disaster recovery, and product behavior. It reports evidence gaps; it does not decide acceptance.
