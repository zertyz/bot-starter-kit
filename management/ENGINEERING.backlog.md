
# Planned

## ER.MCP.02.a-001 -- Update `Mo`, `Mt`, `Party` for their final versions
1. Use the existing Telegram and Whatsapp demoscenes for the refactorings
2. Update those demoscenes to use the new abstractions
3. Check they work exactly as before
==> Luiz. Planned: 2026-07-05;
==> Blocked: 2026-07-18; Reason: Underlying requirement is under revision after the findings of the WhatsApp Demoscene

# Started

## ER.Demo.02-002 -- Complete the WhatsApp message and UI feature comparison
1. Distinguish session interactive reply buttons from approved template quick replies; do not claim the API controls client-side rows, columns, or button iconography.
2. Keep the session `menu` command and add an optional approved-template demonstration for the promotion-style message shown in the supplied screenshot.
3. Exercise and document where WhatsApp accepts emojis and where it does not expose arbitrary icons.
4. Document WhatsApp features missing in relation to Telegram, including delivered text and media editing.
5. Audit the official inbound WhatsApp message types against `whatsapp-business-rs` and document unsupported types without pretending the crate parses them.
6. Keep setup requirements explicit, including the Meta-side template approval needed by the optional template demonstration.
7. Document Meta's current pricing model, the no-charge service-window boundary, and the sending-rate constraints relevant to the demo.
8. Add SQLite, ReDB, and Heed benchmark actions without changing their shared implementations; send the first update, at most one current update per minute, and the final outcome on WhatsApp.
   ==> Luiz. Planned: 2026-07-17; Started: 2026-07-18;

## ER.Demo.01-002 -- Complete Telegram parity demonstrations identified by the WhatsApp comparison
1. Register the bot's native command-list menu so users can open a long option list from Telegram's menu button.
2. Add text and graphical commands for reacting to the user's message.
3. Make the existing text-edit and image-edit demonstrations explicit in the command list and graphical menu.
4. Document any compared WhatsApp behavior that Telegram's Bot API cannot reproduce.
   ==> Luiz. Planned: 2026-07-18; Started: 2026-07-18;


# "In Code Review"

# "Integrated"


# QA

# Merged


# Rolled Out

## EN.Demo.01-001 -- Improve the Telegram Demoscene
1. Messages are not being sent with formatting / html instructions. Fix that.
==> Luiz. Planned: 2026-06-29; Started: 2026-07-03; Merged: 2026-07-04; Rolled Out: 2026-07-04;
==> History exception: 2026-07-15; Missing: In Code Review; Reason: State trail predates deterministic transition enforcement; no date was recorded.

## EN.MCP.01-001 -- Implement "Per User Routing" and adjust the logic to use 1 Stream per user.
1. We don't have a formal common MO, MT, nor User by now.
2. Be creative using what we have -- possibly including generics?
   ==> Luiz. Planned: 2026-06-30; Started: 2026-07-01; Merged: 2026-07-03; Rolled Out: 2026-07-03;
   ==> History exception: 2026-07-15; Missing: In Code Review; Reason: State trail predates deterministic transition enforcement; no date was recorded.

## EN.Gov.01-001 -- Harden the management governance system
1. Add a validated state and management-record model with safe, serialized, atomic mutations.
2. Harden the shared local GUI and static report without splitting their model or frontend.
3. Add a repo-local semantic management skill backed by packets that keep deterministic quality findings, semantic prompts, `work_coverage`, and lifecycle-aware `evidence_coverage` distinct.
4. Add adversarial policy, storage, HTTP, concurrency, export, and browser-facing tests.
5. Keep the current production deployment workflow unchanged, as explicitly directed for this early development phase.
   ==> Codex. Planned: 2026-07-15; Started: 2026-07-15; In Code Review: 2026-07-15; Merged: 2026-07-15; Rolled Out: 2026-07-15;

## EN.Demo.02-001 -- Do the WhatsApp Demoscene version
1. Use the `whatsapp-business-rs` crate and explore everything it has to offer
2. Document any issues found – such as bad security, bad concurrency, poor features support, etc.
   ==> Luiz. Planned: 2026-07-05; Started: 2026-07-06; In Code Review: 2026-07-16; Merged: 2026-07-16; Rolled Out: 2026-07-16;
==> Gate override for Rolled Out: 2026-07-16; Reason: Still in auto-release phase; QA done by Luiz

## EF.Gov.01-002 -- Clarify `block_work` behavior and management command help
Related bug: `BUG-0001`
1. Investigate the reported `block_work ER.MCP.02.a-001` failure, which cited `management/BUSINESS.backlog.md:3` and said a Planned backlog state disagreed with a final recorded Rolled Out state.
2. Confirm that Planned work can be blocked without changing its lifecycle state and retain regression coverage for that behavior.
3. Explain in command help that blocking is an independent condition, full management-model validation precedes mutations, and errors may therefore identify a record unrelated to the requested work item.
4. Make every management command's `--help` state its purpose, when to use it, and representative examples.
5. Restore structured bug-report intake and document `F`-motivation backlog work as the path from a validated report to planned corrective work.
6. Document the commands that create and inspect concrete `TRACEABILITY.md` evidence when an `advance_state` readiness gate fails.
   ==> Codex. Under Planning: 2026-07-18; Planned: 2026-07-18; Started: 2026-07-18; In Code Review: 2026-07-18; QA: 2026-07-18; Merged: 2026-07-18; Rolled Out: 2026-07-18;
   ==> Gate override for Rolled Out: 2026-07-18; Reason: Still in auto-release phase; QA done by Luiz
