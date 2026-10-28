# QuixiDesk provider configuration

The original interface manages providers through `/api/quixi/providers` and the related save, test, OAuth, detection, and onboarding endpoints. Provider implementations and model catalogs live in `crates/quixi-engine/src/providers.rs`.

## Interface boundary

Provider names and valid model identifiers appear in the provider and model selectors. They are integration labels, not application branding. The rest of the interface uses QuixiDesk terminology. Provider cards use simple initials instead of third-party product artwork.

Each configuration form shows a masked credential state, endpoint, and model. A blank credential field preserves a stored key. Endpoint validation accepts HTTP and HTTPS without embedded credentials. Custom endpoints require a model ID and a supported API format.

The Anthropic integration uses its documented Messages API and valid model IDs. These technical identifiers and its provider selector entry remain intentionally supported. Other application assets, typography, tool names, and interface namespaces are independent of that provider.

## Authentication

API keys are entered in password fields. The public provider state omits raw keys and OAuth tokens. Account sign-in is offered only for supported provider integrations. The user opens the provider verification page explicitly; errors and pending states remain visible in the configuration form.

Saving, selecting, testing, and removing a provider are separate operations. A saved configuration is not proof of a working network connection. Tests use synthetic keys against local fixture endpoints.

## Selection and task behavior

The model picker lists configured providers and their models. Selecting a model updates the provider state and the active conversation where applicable. Existing conversation content is not rewritten to remove model or provider names from user-authored text.

## Validation

Verify custom URL/model validation, masked credential handling, unsuccessful connections, local model detection, provider/model switches, streaming, cancellation, and state after restart. Run OAuth approval flows separately from deterministic local tests. Do not use real credentials in the fixture profiles.
