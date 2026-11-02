# Scoped task context

Implemented in `crates/quixi-engine/src/task_context.rs` and integrated in
`crates/quixi-engine/src/lib.rs` (Rust 2021).

## Prompt and tool selection

Each tool-enabled request derives a turn-local context from the current request,
any guidance received at a safe boundary, same-conversation registered artifacts,
and whether the preceding task used Artifact. The same context supplies optional
instructions and filters the model-visible tool catalog. The execution dispatcher
also rejects Artifact when it was not enabled. Model visibility is not permission.

Normal coding, HTML repository files, builds, release artifacts, and publishing
requests do not activate the local-preview feature. An explicit local HTML preview
or artifact creation request does. Registered IDs/titles can activate revisions;
short revision follow-ups can use the preceding artifact task. An unrelated
registered artifact does not automatically enable the feature. No workspace scans
or parent prompt inheritance are involved. Existing child tool allowlists still
exclude Artifact; delegation does not inherit the parent's preview context.

The core prompt states that one tool's limitations do not restrict other tools.
Artifact guidance describes only what that tool does: private local preview
creation, not uploading or deployment. It does not prescribe refusing release
requests. Generated optional sections are rebuilt rather than recovered from old
conversation instructions.

Detection is deliberately conservative English phrase matching, not a semantic
intent classifier. Ambiguous or non-English requests may need an explicit request
such as “Create a local HTML preview.” Merely opening a preview does not currently
establish conversational intent: reference its title/ID or ask for a revision.

## Permissions

`crates/quixi-engine/src/permissions.rs` routes Artifact through the normal input
validation, cancellation, and approval path. Manual and Allow file edits request
approval (previews are not workspace file edits); Auto permits execution. Denial
does not create or update the preview. Existing same-session ID ownership checks
remain in `local_ui.rs`.

## Diagnostics

Open a conversation's **Task actions → Prompt diagnostics** in
`resources/ui/app.js`. The view displays the latest model request (including a separate text-only fallback
snapshot): configuration version, generation path, round, permission mode, section
sources/activation reason, and model-visible tool names. It does not store instruction text or arguments.
The local endpoint is `/api/quixi/prompt-context/{sessionId}`. Snapshots are
in-memory, replaced each round, and unavailable before a model request or after
restart. Text-only fallback snapshots have an empty tool list and omit Artifact and
skill guidance. Standalone text generation has no conversation ID and therefore
no conversation diagnostic snapshot.

## Core prompt and request boundaries

`crates/quixi-engine/src/prompt_context.rs` assembles the shared core, safety,
current permission/capability context, labeled project/workspace instructions,
and optional Artifact/skill guidance, in that order. Tool-enabled and text-only
paths use the same latest-request precedence and trust rules. Text-only adapters
no longer claim filesystem access or completed tool actions.

Assessments and proposals do not authorize implementation or deployment. An
explicit subsequent implementation request authorizes the agreed scope under
current permissions. Historical availability claims and handoffs cannot override
current requests or permissions. These are model instructions, not a new semantic
execution-permission classifier; existing execution gates remain authoritative.

`crates/quixi-engine/src/context.rs` keeps generated historical handoffs and the
verbatim current request in separate user messages during compaction, retaining
the original current-request message when present in the recent tail. Generated
summaries never gain system authority. User-pasted mixed handoffs are not parsed
by brittle heading heuristics; the shared core tells the model to distinguish
current requests from quoted historical context.

See `docs/prompt-behavior-evals.md` for opt-in behavioral probes and their limits.

## Verification

Unit coverage includes ordinary code/HTML/release requests, explicit creation,
registered references, unrelated existing previews, short follow-ups, transcript
boundaries, child-session isolation, real catalog filtering, and manual denial
versus Auto execution. Run `cargo test --workspace`,
`node --test tests/ui/*.test.cjs`, and `make`.
