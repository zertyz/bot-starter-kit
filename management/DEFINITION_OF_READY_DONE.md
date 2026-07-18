# Definition of Ready and Done

These rules define what must be true before a work item changes state. They complement `MANAGEMENT.md`; they do not replace the requirement and backlog files.

`advance_state` enforces the mechanically verifiable subset of these rules. Ordering, dates, ownership, terminal states, and active blockers are hard invariants. A human may override a missing-evidence gate only by recording a reason with `--override-gate`; that history does not replace semantic review or acceptance.

Legacy state trails must not be repaired with invented dates. A dated `History exception` may name an intermediate state whose date was never recorded and explain why; strict lint accepts only necessary, well-formed exceptions and preserves the original trail.


## Ready for Planning

A requirement is ready for planning when:

1. It has a stable requirement ID.
2. It names the expected behavior or operational outcome.
3. Its actor, system boundary, and failure behavior are clear enough to discuss.
4. Known dependencies, risks, and open questions are listed or explicitly marked as unknown.


## Ready to Start

A work item is ready to move from `Planned` to `Started` when:

1. It references one valid requirement.
2. Its branch/work-item ID follows the management grammar.
3. It has one responsible engineer.
4. The expected code, test, documentation, or operational evidence is clear.
5. Any known blocker is recorded as a blocker, not hidden in the description.


## Ready for Code Review

A work item is ready to move to `In Code Review` when:

1. The implementation matches the referenced requirement and work-item description.
2. New or changed behavior has appropriate tests, benchmarks, operational evidence, or documented review evidence.
3. The branch passes formatting, linting, tests, and project rule checks that apply to the changed files.
4. Requirement drift discovered during implementation is reported.
5. Any non-blocking debt introduced by the work is documented with justification and deadline.


## Ready for QA

A work item is ready for `QA` when:

1. Review feedback that changes behavior has been addressed.
2. The acceptance path can be exercised by a human, test, script, or staging evidence.
3. Known limitations are documented as requirement changes, follow-up work, or accepted non-blocking debt.


## Ready to Merge

A work item is ready to move to `Merged` when:

1. It satisfies the definition for code review and, when applicable, QA.
2. Required CI checks pass.
3. The backlog state trail records the dates already reached.
4. The traceability entry can point to the requirement, work item, and evidence.
5. No release-blocking debt is known.

If `advance_state` reports a missing Current Links row or concrete evidence path, use `scripts/management/link_evidence WORK_ITEM_ID EXISTING_REPO_PATH [...]`, verify it with `trace_requirement`, and retry. The complete workflow and examples are in `management/TOOLING.md`.


## Ready to Roll Out

A work item is ready to move to `Rolled Out` when:

1. The release or deployment that contains it is identified.
2. Post-merge checks and release checks passed.
3. Operational evidence exists for behavior that needs staging, monitoring, recovery, or manual acceptance.
4. Rollback or mitigation notes exist when the change can affect production availability or persisted data.

If `advance_state` reports a missing Released entry or operational/release evidence, prepare and execute the approved release, then record the observed outcome with `scripts/management/record_release VERSION WORK_ITEM_ID ...`. Link the resulting ledger with `scripts/management/link_evidence WORK_ITEM_ID management/RELEASES.md`, inspect it with `trace_requirement`, and retry the transition. `record_release` never tags, deploys, links evidence, or advances state; the complete sequence is in `management/TOOLING.md`.


## Done

A work item is done when:

1. Its accepted behavior is present in `main`.
2. The required verification evidence exists.
3. Its backlog state trail includes the final state and date.
4. Related decisions, traceability entries, release notes, and risk records are updated when they apply.
5. No undocumented drift remains between requirements, code, tests, and management records.
