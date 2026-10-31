# Composer delivery controls

The composer has one primary action: Send when idle, Queue while running. Enter follows that action; Shift+Enter inserts a newline. The running-only disclosure beside Queue offers Guide current task. Guidance is consumed at a safe agent-loop boundary, not by interrupting an executing tool.

Pending cards expose Remove and, for queued messages, Guide task. After Stop, pending cards are held and offer Send now when idle. Releasing a held card does not release other held cards. Stop does not clear pending messages; deleting the conversation does. Pending messages remain in memory only and do not survive application restart.

Backend mutations serialize against submission/completion. Removing an already-consumed message returns cancelled:false; promoting a consumed message returns an error. A still-unwinding stopped worker cannot receive explicitly released guidance.

Implementation: resources/ui/app.js, resources/ui/app.css, crates/quixi-engine/src/chat_workflows.rs, crates/quixi-engine/src/lib.rs, crates/quixi-engine/src/storage.rs, crates/quixi-ipc/src/lib.rs.

This is an independent interaction design informed by static observations of held-message cancellation/delivery in Claude.app, not a claim to reproduce its visually verified UI.
