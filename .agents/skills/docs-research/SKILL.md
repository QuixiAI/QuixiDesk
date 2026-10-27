---
name: docs-research
description: Gather official, up-to-date documentation and convert it into a concise implementation checklist for the current coding task.
---

When the user asks for implementation guidance that depends on external docs:

1. Start from official/primary docs first.
2. Capture only task-relevant constraints (versions, limits, required fields, auth, edge cases).
3. Convert findings into a short action checklist with concrete commands or code-level steps.
4. Cite where each critical constraint came from.
5. Call out unknowns explicitly instead of guessing.

Output format:
- `What changed`
- `What matters for this repo`
- `Implementation checklist`
- `Risks/unknowns`
