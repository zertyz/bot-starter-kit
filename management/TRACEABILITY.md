# Traceability

Traceability connects requirements, work items, implementation, and verification evidence. It is a management aid; the requirement files remain authoritative for desired behavior.


## Current Links

| Requirement | Work Item | State | Evidence |
| --- | --- | --- | --- |
| `E.MCP.01` | `EN.MCP.01-001` | Merged | `src/messaging/user_router.rs`; messaging tests in the same module |
| `E.Demo.01` | `EN.Demo.01-001` | Rolled Out | `src/logic/telegram_demoscene.rs`; Telegram gateway code |
| `E.Demo.02` | `EN.Demo.02-001` | Started | No implementation evidence yet |
| `E.MCP.02.a` | `ER.MCP.02.a-001` | Planned | Existing messaging contracts are implementation context; final model update is not complete |
| `E.Gov.01` | `EN.Gov.01-001` | Started | `scripts/management/management_tool`; `scripts/management/gui_server`; `scripts/management/test_management_tool`; `scripts/management/test_gui_server`; `.agents/skills/analyze-management/SKILL.md`; `management/TOOLING.md` |


## Unmapped Requirement Areas

The following requirement areas currently need backlog mapping or evidence review:

1. `B.MsgP.*` -- supported messaging platforms and feature follow-up.
2. `B.UsrMgn.*` -- user-management behavior.
3. `E.MCP.02.b` -- platform and feature inquiry.
4. `E.Arch.*` -- responsibility layers.
5. `O.DS.*` -- data storage and persistence rules.


## Evidence Rules

1. Tests are evidence of verified behavior.
2. Benchmarks are evidence of measured performance behavior.
3. Operations scripts, deployment logs, and monitoring records are evidence of operational behavior.
4. Code in `main` is evidence of actual behavior.
5. Code outside `main` is evidence of behavior under development.
6. Missing evidence does not invalidate a requirement; it identifies a traceability gap.
