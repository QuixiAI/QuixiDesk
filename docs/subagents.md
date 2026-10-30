# Subagent execution

QuixiDesk delegates bounded work through a `Task` tool. A child has its own identity, transcript and lifecycle, attached to the calling conversation and tool invocation. It is not a new top-level conversation and does not appear in Recents.

## Basis for the design

The October 29, 2026 REA inspection of Claude Desktop 4.4212.0 found separate parent/child identities, separate child transcripts, selective forwarding of child output, and permissions owned by the parent. Its shipped desktop code distinguishes delegated work from independently created sessions. Cowork explicitly disables background tasks; the Code integration has different controls. Managed worktrees are conditional, not universal.

Those observations informed the boundaries below. QuixiDesk's code and interface are original. The analysis did not establish universal concurrency limits, recursion depth, context inheritance or runtime defaults; our limits are deliberate product choices, not claims about Claude's behavior.

## Execution contract

`Task` accepts a short `title`, a self-contained `prompt`, and optional `allow_edits` (false by default). Unknown options, including background execution, are rejected. The child receives the explicit assignment, workspace path, project instructions and workspace `AGENTS.md`. It does not receive the parent's conversation transcript. Project and workspace instruction text is bounded to 16,000 and 32,000 characters respectively.

- Foreground, sequential execution: the parent waits for the child result.
- At most four new child tasks per parent turn; no nested delegation.
- At most 12 provider rounds per child, with a 24,000 estimated-token context ceiling and 2 MiB saved-history ceiling. Reaching a limit is a failure with recorded activity, not a successful completion.
- Read-only children have `Read`, `ListDirectory` and `Grep`.
- `allow_edits` additionally enables `Write`, `Edit`, `Bash` and `WebFetch`. This enables tools, not permission to execute them without approval.
- No MCP dispatch, artifact publication, session controls or delegation tools in children. The engine enforces the allowlist even if a provider requests unadvertised tools.
- Children use the existing configured subagent provider/model when available; otherwise they use the parent's model routing. Explicit provider selection keeps duplicate model IDs from switching endpoints. Delegation currently requires the OpenAI-compatible or OpenAI OAuth tool path.

The child uses the parent's current permission mode and cancellation token. Manual mode pauses writes and commands for user approval. Approval cards identify the child. A child assignment or result is never treated as user approval. Auto mode retains its existing broad authority; these restrictions are not an OS sandbox.

Children share the selected workspace. Writes are real and are not rolled back by Stop. Sequential execution avoids sibling write races, but does not provide filesystem isolation. Background workers, parallel delegation, managed worktrees and resumable child conversations are not implemented.

## Storage, results and recovery

Records live under `data/subagents/<parent-hash>/<agent-uuid>.json`. They include the parent tool invocation, assignment, provider/model, timestamps, status, tool counts, individual messages and final summary or error. Writes are atomic. Saving must succeed before a tool can execute; a persistence failure stops further work.

The parent receives a summary capped at 12,000 characters, status, error information and tool counts. Full child messages stay in the separate history. Mutation reports invalidate the parent's tool-result cache, including when a child modifies files and later fails.

The conversation's expandable Subagents panel shows status and opens the saved activity. Stop cancels outstanding approvals and Bash process trees through the existing tool executor. An in-flight provider HTTP request can continue until it returns or times out, but its response cannot authorize further tools after cancellation. Running records recovered after an app exit become `interrupted`; nothing automatically replays. Deleting a parent removes child records and blocks late writes from its worker.

## Verification

Engine fixtures cover the full parent → child → Read → child result → parent answer path; context separation and restart persistence; forced forbidden tools; Stop while approval is pending; crash recovery; deletion and late writes; provider pinning; round/delegation limits; cancellation races; and persistence failures. UI tests cover collapsed activity, transcript separation, owner-scoped retrieval and escaping untrusted child output.

Native macOS verification uses a disposable profile and local scripted provider. It exercises real file reads, a manually approved child write, activity inspection and cancellation. Scripted provider tests establish application behavior; they do not measure the quality of arbitrary models' delegation decisions.
