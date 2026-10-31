//! Turn-local Artifact eligibility, derived only from the supplied request and registry.
//! Callers must supply the current session (including for children), not a parent's
//! context. This heuristic is tool selection, not an authorization boundary.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskContext {
    pub artifact_enabled: bool,
    pub artifact_reason: &'static str,
}

impl TaskContext {
    pub fn derive(
        prompt: &str,
        artifacts: &[Value],
        session_id: &str,
        previous_artifact_turn: bool,
    ) -> Self {
        let text = normalize(prompt);
        let has = |phrase: &str| contains_phrase(&text, phrase);
        let any = |phrases: &[&str]| phrases.iter().any(|phrase| has(phrase));
        // Fail closed on common negations and non-preview meanings of artifact.
        let excluded = any(&[
            "do not",
            "don't",
            "dont",
            "never",
            "without",
            "no artifact",
            "no preview",
            "build artifact",
            "build artifacts",
            "release artifact",
            "release artifacts",
            "ci artifact",
            "ci artifacts",
            "compiled artifact",
            "compiled artifacts",
        ]);
        let creation = any(&["create", "make", "generate", "design", "render"]);
        let preview_target = any(&[
            "local html preview",
            "private html preview",
            "html artifact",
            "local artifact",
            "private artifact",
            "artifact preview",
            "local preview",
            "create an artifact",
            "create a new artifact",
            "make an artifact",
            "generate an artifact",
        ]);
        let same_session: Vec<&Value> = artifacts
            .iter()
            .filter(|artifact| {
                !session_id.is_empty()
                    && artifact.get("createdBySessionId").and_then(Value::as_str)
                        == Some(session_id)
                    && artifact
                        .get("id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| !id.trim().is_empty())
            })
            .collect();
        let revision = any(&[
            "update", "revise", "change", "edit", "adjust", "tweak", "redesign", "restyle",
            "replace", "remove", "add", "make",
        ]);
        let reference = same_session.iter().any(|artifact| {
            ["id", "title"].iter().any(|key| {
                artifact
                    .get(*key)
                    .and_then(Value::as_str)
                    .is_some_and(|value| {
                        let value = normalize(value);
                        !value.is_empty() && contains_phrase(&text, &value)
                    })
            })
        });
        // A previous turn alone must not convert a new coding task into a preview.
        let coding_or_new_task = any(&[
            "code",
            "coding",
            "rust",
            "python",
            "javascript",
            "typescript",
            "cargo",
            "build",
            "compile",
            "release",
            "deploy",
            "test",
            "tests",
            "function",
            "file",
            "files",
            "repository",
            "repo",
            "website",
            "server",
            "api",
            "bug",
            "implement",
            "refactor",
            "new task",
            "instead",
            "unrelated",
            "now write",
            "script",
            "package",
            "dependency",
            "dependencies",
        ]) || prompt.contains('/')
            || prompt.contains('\\')
            || prompt.contains('`');
        let followup_target = any(&["it", "this", "that", "the preview", "the artifact"]);
        let short_followup = text.split_whitespace().count() <= 24
            && prompt.len() <= 240
            && revision
            && followup_target
            && !coding_or_new_task;
        let reason = if excluded {
            None
        } else if creation && preview_target {
            Some("explicit local preview or artifact creation request")
        } else if reference && (revision || any(&["preview", "artifact"])) {
            Some("same-session registered artifact reference")
        } else if previous_artifact_turn && !same_session.is_empty() && short_followup {
            Some("short revision following a same-session artifact turn")
        } else {
            None
        };
        Self {
            artifact_enabled: reason.is_some(),
            artifact_reason: reason.unwrap_or("no explicit or same-session artifact intent"),
        }
    }

    pub fn guidance(&self) -> &'static str {
        if self.artifact_enabled {
            "For the requested local HTML preview, use Artifact to create or revise a private local artifact. When revising, use the registered artifact id from this session. Artifact only creates or updates a local preview; it does not upload, publish, or deploy. This guidance applies only to the requested artifact work; other requested work uses its normal tools and permissions."
        } else {
            ""
        }
    }

    /// Filter a tool-definition array (native or OpenAI function format).
    /// Also accepts an envelope with a `tools` array; unknown shapes are untouched.
    pub fn filter_tools(&self, tools: &mut Value) {
        if self.artifact_enabled {
            return;
        }
        let list = if tools.is_array() {
            tools.as_array_mut()
        } else {
            tools.get_mut("tools").and_then(Value::as_array_mut)
        };
        if let Some(list) = list {
            list.retain(|tool| {
                tool.get("name").and_then(Value::as_str) != Some("Artifact")
                    && tool.pointer("/function/name").and_then(Value::as_str) != Some("Artifact")
            });
        }
    }
}

/// Inspect only the previous user task, skipping tool-result envelopes.
pub fn previous_artifact_turn(transcript: &[Value], current_prompt: &str) -> bool {
    let mut messages = transcript.iter().rev().peekable();
    if messages.peek().is_some_and(|m| m["type"] == "user"
        && m.pointer("/message/content/0/text").and_then(Value::as_str) == Some(current_prompt)) {
        messages.next();
    }
    for message in messages {
        let Some(blocks) = message.pointer("/message/content").and_then(Value::as_array) else { continue };
        if message["type"] == "user" && blocks.iter().any(|b| b["type"] == "text") {
            break;
        }
        if message["type"] == "assistant" && blocks.iter().any(|b| b["type"] == "tool_use" && b["name"] == "Artifact") {
            return true;
        }
    }
    false
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_phrase(text: &str, phrase: &str) -> bool {
    text.match_indices(phrase).any(|(start, _)| {
        (start == 0 || text.as_bytes()[start - 1] == b' ')
            && (start + phrase.len() == text.len() || text.as_bytes()[start + phrase.len()] == b' ')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn previous_turn_skips_results_but_not_unrelated_tasks() {
        let mut transcript = vec![
            json!({"type":"user","message":{"content":[{"type":"text","text":"Create a local preview"}]}}),
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Artifact"}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","content":"Created"}]}}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"Done"}]}}),
            json!({"type":"user","message":{"content":[{"type":"text","text":"Make it blue"}]}}),
        ];
        assert!(previous_artifact_turn(&transcript, "Make it blue"));
        transcript.push(json!({"type":"assistant","message":{"content":[{"type":"text","text":"Other task done"}]}}));
        assert!(!previous_artifact_turn(&transcript, "New task"));
        assert!(!previous_artifact_turn(&[], "Make it blue"));
    }

    #[test]
    fn real_tool_catalog_and_guidance_are_gated_together() {
        for prompt in ["Fix index.html", "Commit, push and publish the release", "Create release artifacts"] {
            let context = TaskContext::derive(prompt, &registry(), "parent", true);
            let mut tools = crate::agent_tools::available_tool_schemas("auto");
            context.filter_tools(&mut tools);
            assert!(context.guidance().is_empty());
            assert!(!tools.as_array().unwrap().iter().any(|t| t["function"]["name"] == "Artifact"));
            assert!(tools.as_array().unwrap().iter().any(|t| t["function"]["name"] == "Bash"));
        }
        let context = TaskContext::derive("Create a local HTML preview", &[], "parent", false);
        let mut tools = crate::agent_tools::available_tool_schemas("auto");
        context.filter_tools(&mut tools);
        assert!(!context.guidance().is_empty());
        assert!(tools.as_array().unwrap().iter().any(|t| t["function"]["name"] == "Artifact"));
    }

    fn registry() -> Vec<Value> {
        vec![json!({"id":"art-123", "title":"Budget Board", "createdBySessionId":"parent"})]
    }

    fn enabled(prompt: &str, previous: bool) -> bool {
        TaskContext::derive(prompt, &registry(), "parent", previous).artifact_enabled
    }

    #[test]
    fn ordinary_work_is_not_an_artifact_request() {
        for prompt in [
            "Write a Rust function",
            "Create index.html and style.css",
            "Build an HTML website",
            "Create an HTML page",
            "Generate release artifacts",
            "Upload build artifacts",
            "Make a local HTML website",
            "Explain what an HTML artifact is",
            "Create a release artifact containing an HTML preview",
            "Do not create a local HTML preview",
            "Create HTML without an artifact",
        ] {
            assert!(!enabled(prompt, false), "{prompt}");
        }
    }

    #[test]
    fn explicit_creation_needs_no_registry_or_history() {
        for prompt in [
            "Create a local HTML preview of a budget",
            "Make an HTML artifact for this chart",
            "Generate a private artifact",
            "Create an artifact showing a timeline",
            "Please create a local preview",
            "CREATE A LOCAL HTML PREVIEW!",
        ] {
            let context = TaskContext::derive(prompt, &[], "child", false);
            assert!(context.artifact_enabled, "{prompt}");
            assert!(context.guidance().contains("does not upload"));
        }
    }

    #[test]
    fn references_are_registered_and_session_scoped() {
        for prompt in [
            "Preview art-123",
            "Revise Budget Board",
            "Show Budget Board artifact",
        ] {
            assert!(enabled(prompt, false), "{prompt}");
            assert!(!TaskContext::derive(prompt, &registry(), "child", false).artifact_enabled);
        }
        for prompt in [
            "Budget Board",
            "Preview unknown-id",
            "Revise Budget Boardroom",
        ] {
            assert!(!enabled(prompt, false), "{prompt}");
        }
        let malformed = vec![json!({"title":"Budget Board", "createdBySessionId":"parent"})];
        assert!(
            !TaskContext::derive("Revise Budget Board", &malformed, "parent", true)
                .artifact_enabled
        );
        assert!(!TaskContext::derive("Preview art-123", &registry(), "", true).artifact_enabled);
    }

    #[test]
    fn short_followups_require_both_continuity_and_local_registration() {
        for prompt in [
            "Make it blue",
            "Add a heading to it",
            "Change the preview background",
        ] {
            assert!(enabled(prompt, true), "{prompt}");
            assert!(!enabled(prompt, false), "{prompt}");
            assert!(!TaskContext::derive(prompt, &[], "parent", true).artifact_enabled);
        }
    }

    #[test]
    fn unrelated_new_tasks_and_children_do_not_inherit_eligibility() {
        for prompt in [
            "Make it compile",
            "Update the Rust code",
            "Add tests for it",
            "Make this website",
            "Change this file",
            "Now write a script",
            "New task: make it blue",
            "Fix the API bug",
            "Update src/main.rs",
        ] {
            assert!(!enabled(prompt, true), "{prompt}");
        }
        // Even an accidentally forwarded previous-turn flag cannot reuse parent records.
        assert!(!TaskContext::derive("Make it blue", &registry(), "child", true).artifact_enabled);
        assert!(
            !TaskContext::derive("Revise Budget Board", &registry(), "child", true)
                .artifact_enabled
        );
    }

    #[test]
    fn guidance_and_filtering_are_scoped() {
        let off = TaskContext::derive("Write code", &[], "s", false);
        assert_eq!(off.guidance(), "");
        let original = json!([
            {"name":"Artifact"}, {"type":"function", "function":{"name":"Artifact"}},
            {"name":"Read"}, {"name":"ArtifactExport"}, null
        ]);
        let mut tools = original.clone();
        off.filter_tools(&mut tools);
        assert_eq!(
            tools,
            json!([{"name":"Read"}, {"name":"ArtifactExport"}, null])
        );
        let mut envelope = json!({"tools":original.clone(), "metadata":"keep"});
        off.filter_tools(&mut envelope);
        assert_eq!(envelope["tools"], tools);
        assert_eq!(envelope["metadata"], "keep");
        let on = TaskContext::derive("Create an HTML artifact", &[], "s", false);
        let mut untouched = original.clone();
        on.filter_tools(&mut untouched);
        assert_eq!(untouched, original);
        for mut value in [Value::Null, json!({"tools":false}), json!("Artifact")] {
            let before = value.clone();
            off.filter_tools(&mut value);
            assert_eq!(value, before);
        }
    }
}
