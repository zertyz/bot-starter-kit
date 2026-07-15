# Instructions for Codex AI Agents

## Change scope and intent preservation

1. Preserve intent. Do not delete code when doing so would discard user intent. If an in-scope change requires deletion and the intent is ambiguous, ask first.
2. Preserve comments and string literals unless changing them is required by the request or necessary to keep them accurate after an in-scope change. Ask before changing user-facing & API-facing text or established contracts when compatibility or intended wording is unclear.
3. Make only the requested change and its necessary supporting changes. Use engineering judgment within that boundary; ask before materially expanding scope.
4. Documentation must earn its place: infer its purpose, write expressive and succinct prose, and omit generic caveats or filler.

## Engineering judgment

1. Infer the smallest coherent domain model for the requested change.
2. Evaluate maintainability and likely evolution before selecting an implementation.

## Knowledge Base
Read KNOWLEDGE_BASE.md for instructions.
