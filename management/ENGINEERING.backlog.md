
# Planned

## ER.MCP.02.a-001 -- Update `Mo`, `Mt`, `Party` for their final versions
1. Use the existing Telegram and Whatsapp demoscenes for the refactorings
2. Update those demoscenes to use the new abstractions
3. Check they work exactly as before
==> Luiz. Planned: 2026-07-05;


# Started

## EN.Demo.02-001 -- Do the WhatsApp Demoscene version
1. Use the `whatsapp-business-rs` crate and explore everything it has to offer
2. Document any issues found – such as bad security, bad concurrency, poor features support, etc.
   ==> Luiz. Planned: 2026-07-05; Started: 2026-07-06


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

   