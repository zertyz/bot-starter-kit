# Traceability

Traceability connects requirements, work items, implementation, and verification evidence. It is a management aid; the requirement files remain authoritative for desired behavior. Only parsed rows in the single canonical `## Current Links` table supply management evidence. The column names and order are canonical; Markdown cell padding, separator widths, and alignment markers are presentation details.


## Current Links

| Requirement | Work Item | State       | Evidence                                                                                    |
| --- | --- |-------------|---------------------------------------------------------------------------------------------|
| `E.MCP.01` | `EN.MCP.01-001` | Rolled Out  | `src/messaging/user_router.rs`; messaging tests in the same module                          |
| `E.Demo.01` | `EN.Demo.01-001` | Rolled Out  | `src/logic/telegram_demoscene.rs`; Telegram gateway code                                    |
| `E.Demo.02` | `EN.Demo.02-001` | Started     | `examples/whatsapp_demoscene.rs`; focused tests in the same example                          |
| `E.MCP.02.a` | `ER.MCP.02.a-001` | Planned     | Existing messaging contracts are implementation context; final model update is not complete |
| `E.Gov.01` | `EN.Gov.01-001` | Rolled Out  | `scripts/management`; Issues found by Luiz, then fixed by Codex                             |


## Unmapped Requirement Areas

The following entries acknowledge requirement areas whose work or evidence coverage still needs review. They are coverage metadata: they do not create work mappings, are not traceability rows, and never supply implementation or verification evidence.

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
6. A row's requirement must exactly match its work item's governing requirement. Parent requirement views may aggregate rows belonging to descendants, but the row itself remains attached to the child.
7. Evidence coverage is lifecycle-aware, with `gap` taking precedence when completed work lacks required evidence and `pending` taking precedence while any work remains active. Otherwise use `evidenced` for completed work with concrete existing evidence in a parsed row and `not_applicable` when no lifecycle evidence obligation exists.
8. Missing evidence does not invalidate a requirement or prove that implementation is absent.
