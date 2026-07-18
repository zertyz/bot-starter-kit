# Bug Reports

This ledger separates open, unresolved reports from optional addressed history. A report is intake, not a confirmed defect, requirement, incident, or unit of planned work. It may be filed before anyone knows which management area it affects.

Only entries inside the single exact `## Open Reports` section are active reports consumed by validation, status summaries, and the GUI. Other level-two sections, including `## Addressed Reports`, may retain useful history and are excluded from open-report status and schema rules.

Within `## Open Reports`, use `Reported` while awaiting triage, `Investigating` while establishing what happened, and `Validated` after confirming that corrective work is needed. A validated report links to at least one `F`-motivation work item. Keep the report open until its linked corrective work reaches `Rolled Out`; then move it to an addressed section or delete it when it is no longer useful. Git history also retains the resolved report.

Each open report has a stable `BUG-NNNN` identifier, `Status`, `Reported` date, `Reporter`, `Related work`, and the reporter's original `Report`. A validated report also records its `Assessment`. Use `None` for `Related work` until work is planned.


## Open Reports


## Addressed Reports

### BUG-0001 -- MANAGEMENT: `block_work` clarifications + missing help docs

Status: Rolled Out
Reported: 2026-07-18
Reporter: Luiz
Related work: `EF.Gov.01-002`

Report:

> A tooling behavior seems to be generating unexpected results.
> My intention: to mark a work item (currently in the planned phase) as blocked due to a requirement issue that was recently found.
>
> The work item in question is ER.MCP.02.a-001.
>
> The command I ran and negation:
> ```
> [luiz@HpLap bot-starter-kit]$ ./scripts/management/block_work ER.MCP.02.a-001 --reason "Underlying requirement is under revision after the findings of the WhatsApp Demoscene"
> ERROR: management/BUSINESS.backlog.md:3: current backlog state Planned is not the final recorded lifecycle state Rolled Out
> ```
>
> That command was the one to use to move the state, no? Otherwise, please clarify what that command is for.
>
> Actually, the help of each command is not stating what they should do and when to use it -- maybe with some examples. Lets take this opportunity to fix that gap as well.

Assessment:

The report validated a management-tool clarity and discoverability problem. `block_work` is the correct command and adds an independent blocked condition without moving lifecycle state. The reported error came from the command's full-model validation before mutation. Corrective work `EF.Gov.01-002` reached `Rolled Out`, so this report is retained here only as addressed history.
