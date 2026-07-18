# Existing Bugs

## MANAGEMENT: `block_work` clarifications + missing help docs

A tooling behavior seems to be generating unexpected results.
My intention: to mark a work item (currently in the planned phase) as blocked due to a requirement issue that was recently found.

The work item in question is ER.MCP.02.a-001.

The command I ran and negation:
```
[luiz@HpLap bot-starter-kit]$ ./scripts/management/block_work ER.MCP.02.a-001 --reason "Underlying requirement is under revision after the findings of the WhatsApp Demoscene"
ERROR: management/BUSINESS.backlog.md:3: current backlog state Planned is not the final recorded lifecycle state Rolled Out

That command was the one to use to move the state, no? Otherwise, please clarify what that command is for.

Actually, the help of each command is not stating what they should do and when to use it -- maybe with some examples. Lets take this opportunity to fix that gap as well.
```