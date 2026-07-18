---
name: analyze-management
description: Analyze this repository's management state using deterministic governance evidence plus the actual requirements, backlog, traceability, implementation, tests, and git history. Use for requirement-drift audits, estimates, plan evaluation, implementation reviews, evidence verification, backlog synchronization, or technical-debt analysis; especially when a lexical management command cannot make the requested semantic judgment.
---

# Analyze Management

Ground every conclusion in repository evidence. Treat deterministic tooling as a source of facts and signals, not as a substitute for inspecting the implementation or exercising engineering judgment.

## Run the Workflow

1. Work from the repository root. Read `AGENTS.md`, `KNOWLEDGE_BASE.md`, and the management documents in the order specified by `management/README.ai.md`.
2. Identify the requested workflow and its governing requirement or work item. Read the matching section in [references/workflows.md](references/workflows.md).
3. Run `scripts/management/management_tool lint --strict`. Report structural failures before attempting semantic analysis.
4. Run `scripts/management/semantic_context <workflow>` with the narrowest relevant requirement, work-item, or path filters. If the wrapper is unavailable, gather the same evidence directly and state that fallback.
5. Interpret its dimensions independently: deterministic quality findings are observed facts; semantic prompts require judgment; `work_coverage` describes planning; `evidence_coverage` describes lifecycle evidence. When a focused audit includes descendants, keep the parent and each child conclusion distinct.
6. Inspect the cited source, tests, configuration, and relevant git changes yourself. Do not infer implementation or verification from identifier matches alone.
7. Separate the result into observed evidence, engineering inference, and unknowns. Cite repository paths and commands for material claims.
8. Prioritize findings by impact on the governing requirement. State whether each finding is a requirement gap, implementation gap, verification gap, traceability gap, process risk, or optional improvement.
9. Recommend the smallest coherent next action. Do not rewrite requirements, change acceptance criteria, or declare final acceptance unless the human owner explicitly asks for that action.

## Apply Guardrails

- Preserve the distinction between lifecycle state and the independent blocked condition.
- Treat `management/BUGS.md`'s exact `## Open Reports` section as unresolved report intake, not confirmed defects or planned work. A validated report links to `F` work and remains open until that work reaches `Rolled Out`; do not erase the observation when planning its correction. Other level-two sections are inactive history and may retain addressed reports.
- Never turn an `active` lexical prompt into a semantic conclusion without inspecting context. Treat an `acknowledged` prompt as an explicit author disposition; mixed plain occurrences remain evidence to inspect, not grounds to reactivate it automatically.
- Do not call zero work requirement drift. Describe `work_coverage: unplanned` as planning state unless a separate commitment or historical inconsistency makes it drift.
- Treat only rows parsed from the single canonical `## Current Links` table as traceability evidence. Each row belongs to the exact requirement governing its work item; parent views may aggregate descendant rows. Unmapped Requirement Areas are coverage acknowledgements, not evidence.
- Interpret evidence status by lifecycle: `gap` has precedence when completed work lacks required evidence; otherwise `pending` has active work, `evidenced` has completed work with concrete evidence, and `not_applicable` has no current evidence obligation.
- Treat missing evidence as unknown, not proof that work is absent or incorrect.
- Do not present estimates as facts. Name assumptions and give ranges when uncertainty is material.
- Do not claim a requirement is satisfied solely because lint passes, a work item reached a state, or a traceability row exists.
- Do not modify repository state during an analysis-only request. If changes are requested, keep them within an existing work item or surface the need for one.
- Respect the accepted temporary deployment-policy divergence recorded in `management/DECISIONS.md`; do not propose changing deployment triggers unless the human reopens that decision.

## Report Concisely

Lead with the decision-relevant conclusion. Follow with prioritized findings, supporting evidence, unknowns, and the recommended next action. Say explicitly when no material drift or gap was found.
