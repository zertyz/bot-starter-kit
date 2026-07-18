
# Planned

## ER.MCP.02.a-001 -- Update `Mo`, `Mt`, `Party` for their final versions
1. Use the existing Telegram and Whatsapp demoscenes for the refactorings
2. Update those demoscenes to use the new abstractions
3. Check they work exactly as before
==> Luiz. Planned: 2026-07-05;

## EN.Demo.02-002 -- The WhatsApp Demoscene is not fully exploring all the message types WhatsApp can deliver
1. Please see the "Espaçolaser RJ - Shop Rio Sul" screenshot, proving WhatsApp do support "telegram-menu-like" messages, using two columns
2. Please compare that to the screenshots showing what the current WhatsApp Demoscene is presenting, using a single column
3. Research the docs and solve the gap: change the demoscene "menu" command to offer a 2 columns row and a 1 colum row containing the existing options
4. Do additional resources and see if emojis / icons can be presented there, exactly as we do for Telegram
5. Go ahead and cover additional gaps between WhatsApp and Telegram, like message editing -- for text and image
6. Document -- in the Demoscene -- what messaging and UI features WhatsApp is missing in relation to Telegram.
7. Now go ahead and treat the Telegram code. Check if the existing Telegram code can do everything WhatsApp is doing. Including:
   * Offering a pop-up to select one among a long list of options -- In the WhatsApp demoscene this is called "Features" and is presented with an icon
   * The WhatsApp demoscene do demonstrate a "reaction to a message". Make sure our Telegram code is demoing doing the same.
   * If Telegram cannot do something WhatsApp can, please document it in the telegram demoscene code.
   ==> Luiz. Planned: 2026-07-17
   

# Started


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


   