# QuixiDesk product design

## Product purpose

Provide a native workspace where people can ask questions, inspect local files, and work with optional model providers. Keep project context, conversation history, and private artifacts accessible without exposing implementation details in the main flow.

## Interface identity

The original interface uses a mint Q mark, a slate workspace palette, system typography, and simple outlined SVG controls. Light and dark themes share a consistent spacing scale and clear focus states. Product copy uses the QuixiDesk name. Model and provider names are confined to provider configuration and selection.

The sidebar contains new work, search, projects, artifacts, schedules, tools, and local conversation history. The conversation area distinguishes user messages, assistant text, thinking state, and tool results. A bottom composer preserves unsent text while users navigate between conversations.

## Interaction requirements

- Keep entered data after recoverable failures.
- Show permission state before tools run. Preserve denied-action errors in the transcript.
- Keep cancellation available during a running task.
- Preserve scroll position when the user reads earlier messages.
- Keep project instructions attached to the intended project and folder.
- Confirm destructive actions with specific object names.
- Treat user text and artifact HTML as untrusted content.
- Provide loading, empty, error, and success states that describe actual backend outcomes.

## Storage and privacy

Conversations are local records. Prompts and selected task context are transmitted to the configured model endpoint when the user sends them. Provider configuration is separate from conversation text. Private artifacts are sandboxed local previews. Saving failures stay visible, and closing is blocked if the only conversation copy is in memory.

## Platform behavior

The native host uses WKWebView, WebKitGTK, or WebView2. Shell execution uses a platform-appropriate executable. macOS-specific integrations remain clearly separate from cross-platform workspace features. Manual path entry provides an alternative when a native folder picker is unavailable.

## Honest capability states

A configured external tool is not automatically a verified connection. A saved schedule definition is not evidence that automatic dispatch is implemented. The interface must identify unsupported features instead of reporting synthetic success.

## Validation

Use disposable app profiles and synthetic credentials. Verify conversation streaming, cancellation, provider changes, project context, artifact isolation, search, recovery, and native close behavior. Inspect screenshots at narrow and wide sizes and keep backend and interface tests separate.
