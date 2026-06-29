# Instructions for Codex AI Agents

## Behavior correction for Codex 5.5 Extra-high
1. Never drop code -- even if wrong -- when this would mean dropping intention. Ask first.
2. Never drop comments and do not change Strings, even if they are wrong. Ask first.
3. Do not make unrelated changes. Only make changes to parts you are 100% sure to be what the human wants -- do not invent unrelated scope; within the requested scope, still use engineering judgment. Ask for anything else.
4. When you are asked to add or complete the documentation, make sure every word has real value instead of being verbose or adding typical AI "I can never be wrong" disclaimers. Instead, go infer the intention of having the documentation there and be expressive and succinct, exactly as good code should be.

## Performance Considerations
1. Spend effort inferring the minimal domain model that makes the requested change coherent -- maybe using Mathematical Inference-like capabilities.
2. For every solution (and/or details of the solution) you are considering, spend time simulating future evolutions, considering maintainability, and other engineering practices. Always prefer the solutions more in tune with good engineering practices.

## Knowledge Base
Read KNOWLEDGE_BASE.md for instructions.
