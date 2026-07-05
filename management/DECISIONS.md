# Decisions

Accepted decisions are appended here and kept under version control. If a decision stops being correct, add a superseding decision instead of deleting the old one.


## DEC-0001 -- Requirements and Backlogs Are Git-Controlled Management Sources

Status: Accepted
Date: 2026-07-05

Context:
The project already separates desired behavior, planned work, implementation, and verification evidence. Requirements live in `BUSINESS.md`, `ENGINEERING.md`, and `OPERATIONS.md`; work items live in the corresponding backlog files.

Decision:
Desired behavior is governed by the requirement files. Work is governed by backlog items that reference those requirements. Code, tests, diagrams, and release notes must be reconciled with those sources when they disagree.

Consequences:
Requirement drift is a first-class management state. Humans own requirement changes; automation may detect and report drift.

Related:
`MANAGEMENT.md`, `BUSINESS.md`, `ENGINEERING.md`, `OPERATIONS.md`, `*.backlog.md`


## DEC-0002 -- Messaging Logic Uses Per-User MO and MT Streams

Status: Accepted
Date: 2026-07-05

Context:
The project defines Mobile Originated messages as inbound streams and Mobile Terminated messages as outbound streams. Per-user routing is already listed as done in `ENGINEERING.backlog.md` and implemented in the messaging layer.

Decision:
Dialog logic receives one MO stream per user and returns one MT stream per user. Messaging platform gateways own the routing between platform-level streams and user-level streams.

Consequences:
The core dialog logic can reason about one user at a time. Platform gateways remain responsible for fan-in, fan-out, throttling, and lifecycle behavior.

Related:
`ENGINEERING.md`, `ENGINEERING.backlog.md`, `src/messaging/user_router.rs`


## DEC-0003 -- Dialog Logic Should Be Messaging-Platform Agnostic by Default

Status: Accepted
Date: 2026-07-05

Context:
The business requirements expect several messaging platforms. The engineering requirements define common MO, MT, and user models, with platform and feature inquiry when logic must branch.

Decision:
Common bot behavior should target shared messaging contracts. Platform-specific behavior is allowed when the logic explicitly inquires about platform identity or available features.

Consequences:
New platform support should normally extend gateways and contracts before duplicating dialog logic.

Related:
`BUSINESS.md`, `ENGINEERING.md`, `src/messaging/contracts/`


## DEC-0004 -- Demoscenes Drive Messaging-Layer Refactoring

Status: Accepted
Date: 2026-07-05

Context:
The engineering requirements describe platform-specific demoscenes as drivers for the messaging layer. Current backlog items include Telegram improvement and the first Whatsapp demoscene.

Decision:
Each supported messaging platform may have one platform-specific demoscene. Demoscenes should preserve platform freedom and should not be forced through the common messaging layer before the abstractions are ready.

Consequences:
Refactoring pressure comes from real platform examples, not only from imagined common abstractions.

Related:
`ENGINEERING.md`, `ENGINEERING.backlog.md`, `src/logic/telegram_demoscene.rs`


## DEC-0005 -- Code Is Organized by Responsibility Layers

Status: Accepted
Date: 2026-07-05

Context:
The engineering requirements describe `messaging`, `db`, and `logic` responsibility layers, with dependency inversion to keep behavior testable.

Decision:
New implementation should preserve the responsibility boundaries described in `ENGINEERING.md` and the project-specific patterns in `CODE_PATTERNS.md`.

Consequences:
Shared contracts belong near the layer boundary. Platform, storage, and domain choices should not leak across layers without an explicit requirement or decision.

Related:
`ENGINEERING.md`, `CODE_PATTERNS.md`, `src/messaging/`, `src/db/`, `src/logic/`


## DEC-0006 -- Persistent Data Is Classified by Operational Consequence

Status: Accepted
Date: 2026-07-05

Context:
Operations requirements distinguish application config, session data, and formally persisted data.

Decision:
Storage design must preserve the distinction between disposable session recovery data and data whose loss would degrade service quality or revenue.

Consequences:
Formally persisted data needs stronger inspectability, mutability, and operational care than session data.

Related:
`OPERATIONS.md`, `src/db/`, `src/repository/`


## DEC-0007 -- Production Code Avoids Hidden Error Context and Panic Shortcuts

Status: Accepted
Date: 2026-07-05

Context:
`CODE_GUIDELINES.md` and `scripts/ci/enforce_code_guidelines` already define executable code rules around hidden `anyhow::Context` usage and panic shortcuts in production Rust source.

Decision:
CI should keep enforcing the executable code guidelines. Management tooling should follow the same pattern: written rule first, deterministic checker where possible.

Consequences:
Rules should be specific enough to check and failures should point back to the written rule.

Related:
`CODE_GUIDELINES.md`, `scripts/ci/enforce_code_guidelines`


## DEC-0008 -- Main Is Release-Candidate Quality

Status: Accepted
Date: 2026-07-05

Context:
`MANAGEMENT.md` defines `main` as production-grade, staging-deployable code, with production rollout controlled by release tags and post-merge checks.

Decision:
Known release-blocking debt must not enter `main`. Non-blocking debt that affects future foundations must be documented with justification and a deadline.

Consequences:
Feature branches may contain behavior under development. `main` should not rely on undocumented exceptions to the management rules.

Related:
`MANAGEMENT.md`, `.github/workflows/main.yaml`, `.github/workflows/branch.yaml`
