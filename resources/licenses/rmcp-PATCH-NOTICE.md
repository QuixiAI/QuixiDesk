# Local rmcp 3.8.0 patch

Source: the published `rmcp` 3.8.0 crate from https://github.com/modelcontextprotocol/rust-sdk (Apache-2.0; original LICENSE retained).

`src/model/elicitation_schema.rs` retains the original JSON schema when deserializing an `ElicitationSchema`, exposed through `original_json_schema()`. QuixiDesk uses this input snapshot for server elicitation validation. It does not change the SDK's typed builders or their serialization. Callers that modify typed fields must not treat the original snapshot as their edited schema.

The 3.8.0 typed schema omits string `pattern`, array `uniqueItems`, and unknown constraint keywords. Its schema root also discards unknown keywords, potentially removing restrictions before host validation. Keeping the exact received schema lets the host validate supported constraints and reject unsupported/reference/nested forms, instead of silently weakening them. Known fields such as length, numeric bounds, formats, required properties, titled enum choices, and item bounds continue to use the SDK's existing typed parsing.

Reference: https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation

Packaging intentionally omits upstream integration fixtures/tests and the upstream build script, which configures Git hooks when vendored into a Git workspace. The normalized Cargo manifest disables the build script and omits those test targets; all runtime source and original license are retained. Remove this patch once an upstream release exposes lossless original elicitation schemas, updating the actual-wire regression tests at the same time.
