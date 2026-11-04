use crate::QuixiEngine;
use serde_json::{json, Value};
use std::sync::Arc;

const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "of", "to", "in", "on", "at", "for", "with", "from", "by", "about", "into",
    "onto", "as", "is", "are", "was", "were", "be", "been", "being", "it", "its", "this", "that",
    "these", "those", "my", "our", "your", "their", "me", "us", "we", "i", "you", "they", "them",
    "not", "no", "good", "bad", "like", "should", "would", "could", "can", "will", "may", "might",
    "must", "just", "really", "very", "some", "any", "all", "both", "each", "how", "what", "why",
    "when", "where", "who", "which", "here", "there", "have", "has", "had", "do", "does", "did",
    "and", "or", "but", "so", "if", "then", "than", "also", "too", "now", "still", "please",
    "let", "lets", "make", "easier", "possible", "sure", "help", "want", "need", "continue",
    "work", "working", "explore", "exploring", "read", "reading", "look", "looking", "check",
    "checking", "inspect", "take", "see", "using", "used", "use", "via", "new", "task", "session",
];

const BOILERPLATE_PATH_STEMS: &[&str] = &[
    "handoff", "readme", "agents", "todo", "todos", "notes", "plan", "changelog", "license",
    "makefile", "cargo", "package", "tsconfig", "index", "lib", "main", "mod", "app", "src",
    "crates", "packages", "docs", "proposals", "test", "tests", "bin", "bins", "dist", "out",
    "blob", "tree", "master", "pull", "issues", "commits", "commit", "users", "home",
];

const CONVERSATIONAL_PREFIXES: &[&str] = &[
    "please read handoff.md and continue our work on ",
    "please read handoff.md and ",
    "read handoff.md and continue our work on ",
    "read handoff.md and ",
    "continue our work on ",
    "continue working on ",
    "make it easier to ",
    "make it possible to ",
    "i would like you to ",
    "i'd like you to ",
    "i would like to ",
    "i'd like to ",
    "i want you to ",
    "i want to ",
    "i need you to ",
    "i need to ",
    "we need to ",
    "can you please ",
    "could you please ",
    "would you please ",
    "can you help me ",
    "could you help me ",
    "help me to ",
    "help me ",
    "can you ",
    "could you ",
    "would you ",
    "will you ",
    "please explore ",
    "please read ",
    "please check ",
    "please inspect ",
    "please review ",
    "please ",
    "let's ",
    "lets ",
    "take a look at ",
    "look into ",
    "check out ",
];

const TITLE_SYSTEM_PROMPT: &str = "\
Write a razor-sharp 1 to 3 word title for this workspace session.
Rules:
- Output ONLY the title on a single line: 1, 2, or 3 words maximum.
- Nail the specific technical subject, feature, bug, or repository.
- Never echo conversational lead-ins (\"Please read\", \"Please explore\", \"Make it easier\", \"Help me\", \"Here is\").
- Prefer crisp Title Case noun phrases (e.g. \"Session Titles\", \"DeepSeek Harness\", \"Bridge Impl Gap\", \"PTY Race Fix\", \"Context Compaction\", \"Seatbelt Sandbox\").
- Preserve canonical casing for acronyms and product/platform terms (MCP, PTY, LSP, UI, IPC, SSE, CLI, API, macOS, iOS, Git, QuixiDesk, DeepSeek).
- No quotes, no markdown emphasis, no trailing punctuation, and no \"Title:\" label.";

fn canonical_word_case(raw: &str) -> String {
    let trimmed = raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '+' && c != '#');
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "mcp" => return "MCP".into(),
        "pty" | "ptys" => return if lower == "ptys" { "PTYs" } else { "PTY" }.into(),
        "lsp" => return "LSP".into(),
        "ui" => return "UI".into(),
        "ux" => return "UX".into(),
        "ipc" | "eipc" => return lower.to_ascii_uppercase(),
        "sse" => return "SSE".into(),
        "cli" => return "CLI".into(),
        "api" | "apis" => return if lower == "apis" { "APIs" } else { "API" }.into(),
        "llm" | "llms" => return if lower == "llms" { "LLMs" } else { "LLM" }.into(),
        "dag" | "dags" => return if lower == "dags" { "DAGs" } else { "DAG" }.into(),
        "ast" => return "AST".into(),
        "sdk" => return "SDK".into(),
        "sql" => return "SQL".into(),
        "ssh" => return "SSH".into(),
        "tls" => return "TLS".into(),
        "http" | "https" => return lower.to_ascii_uppercase(),
        "json" => return "JSON".into(),
        "html" => return "HTML".into(),
        "css" => return "CSS".into(),
        "svg" => return "SVG".into(),
        "pdf" => return "PDF".into(),
        "cpu" | "gpu" => return lower.to_ascii_uppercase(),
        "pr" | "prs" => return if lower == "prs" { "PRs" } else { "PR" }.into(),
        "ci" => return "CI".into(),
        "os" => return "OS".into(),
        "macos" => return "macOS".into(),
        "ios" => return "iOS".into(),
        "ipados" => return "iPadOS".into(),
        "webkit" => return "WebKit".into(),
        "wkwebview" => return "WKWebView".into(),
        "github" => return "GitHub".into(),
        "gitlab" => return "GitLab".into(),
        "openai" => return "OpenAI".into(),
        "deepseek" => return "DeepSeek".into(),
        "quixidesk" => return "QuixiDesk".into(),
        "quixi" => return "Quixi".into(),
        "nodejs" => return "Node.js".into(),
        "javascript" => return "JavaScript".into(),
        "typescript" => return "TypeScript".into(),
        _ => {}
    }

    // Preserve deliberate mixed/camel/acronym casing already in the user's token.
    let has_upper = trimmed.chars().skip(1).any(|c| c.is_uppercase());
    let all_upper_short =
        trimmed.len() <= 5 && trimmed.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
    if has_upper || (all_upper_short && trimmed.len() >= 2) {
        return trimmed.to_string();
    }

    let mut chars = trimmed.chars();
    match chars.next() {
        Some(first) => {
            let mut out: String = first.to_uppercase().collect();
            out.push_str(&chars.as_str().to_lowercase());
            out
        }
        None => String::new(),
    }
}

fn strip_pasted_and_code_blocks(prompt: &str) -> String {
    let mut cleaned = String::with_capacity(prompt.len().min(1600));
    let mut in_fence = false;
    let mut in_tag_block = false;
    for line in prompt.lines().take(40) {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if trimmed.starts_with("<pasted_content") || trimmed.starts_with("<system-reminder") {
            in_tag_block = true;
            continue;
        }
        if trimmed.starts_with("</pasted_content>") || trimmed.starts_with("</system-reminder>") {
            in_tag_block = false;
            continue;
        }
        if !in_fence && !in_tag_block && !trimmed.is_empty() {
            if !cleaned.is_empty() {
                cleaned.push(' ');
            }
            cleaned.push_str(trimmed);
            if cleaned.len() >= 1200 {
                break;
            }
        }
    }
    if cleaned.is_empty() {
        prompt.chars().take(600).collect()
    } else {
        cleaned
    }
}

fn strip_parentheticals(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut depth = 0usize;
    for ch in input.chars() {
        match ch {
            '(' | '[' => depth = depth.saturating_add(1),
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

fn singularize_plural_head(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    if lower.len() > 4
        && lower.ends_with('s')
        && !lower.ends_with("ss")
        && !lower.ends_with("us")
        && !lower.ends_with("is")
        && !lower.ends_with("os")
    {
        if let Some(stem) = lower.strip_suffix("ies") {
            return format!("{stem}y");
        }
        return word[..word.len() - 1].to_string();
    }
    word.to_string()
}

fn slug_words(segment: &str) -> Vec<String> {
    let stem = segment
        .rsplit('/')
        .next()
        .unwrap_or(segment)
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.');
    let stem = match stem.rsplit_once('.') {
        Some((base, ext)) if ext.len() <= 5 && !base.is_empty() => base,
        _ => stem,
    };
    if stem.is_empty() {
        return Vec::new();
    }
    let stem_lower = stem.to_ascii_lowercase();
    if BOILERPLATE_PATH_STEMS.contains(&stem_lower.as_str()) {
        return Vec::new();
    }
    stem.split(|c: char| c == '-' || c == '_' || c == '.')
        .filter(|part| !part.is_empty())
        .filter(|part| {
            let l = part.to_ascii_lowercase();
            !STOP_WORDS.contains(&l.as_str()) && !BOILERPLATE_PATH_STEMS.contains(&l.as_str())
        })
        .map(canonical_word_case)
        .filter(|w| !w.is_empty())
        .take(3)
        .collect()
}

fn extract_salient_path_or_repo_words(text: &str) -> Vec<String> {
    for raw_token in text.split_whitespace() {
        let tok = raw_token.trim_matches(|c: char| {
            matches!(c, '"' | '\'' | '`' | '(' | ')' | '[' | ']' | ',' | ';' | ':' | '.')
        });
        if tok.starts_with("http://") || tok.starts_with("https://") {
            let without_scheme = tok
                .strip_prefix("https://")
                .or_else(|| tok.strip_prefix("http://"))
                .unwrap_or(tok);
            let path_part = without_scheme
                .split(['?', '#'])
                .next()
                .unwrap_or(without_scheme);
            let segments: Vec<&str> = path_part.split('/').filter(|s| !s.is_empty()).collect();
            // Walk segments from tail to find the most specific repo or document slug.
            for seg in segments.iter().skip(1).rev() {
                if seg.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let words = slug_words(seg);
                if !words.is_empty() {
                    return words;
                }
            }
            if let Some(host) = segments.first() {
                let host_label = host
                    .trim_start_matches("www.")
                    .split('.')
                    .next()
                    .unwrap_or("");
                let words = slug_words(host_label);
                if !words.is_empty() {
                    return words;
                }
            }
        } else if tok.contains('/') || tok.starts_with("~/") {
            let segments: Vec<&str> = tok
                .trim_start_matches("~/")
                .split('/')
                .filter(|s| !s.is_empty() && *s != "." && *s != "..")
                .collect();
            for seg in segments.iter().rev() {
                let words = slug_words(seg);
                if !words.is_empty() {
                    return words;
                }
            }
        } else if let Some((base, ext)) = tok.rsplit_once('.') {
            if matches!(
                ext.to_ascii_lowercase().as_str(),
                "rs" | "ts" | "tsx" | "js" | "cjs" | "mjs" | "py" | "go" | "md" | "toml" | "json" | "yaml" | "yml" | "css" | "html" | "sh"
            ) {
                let words = slug_words(base);
                if !words.is_empty() {
                    return words;
                }
            }
        }
    }
    Vec::new()
}

/// Deterministic 1–3 word Title Case fallback computed in <1ms without network I/O.
pub fn smart_fallback_title(prompt: &str) -> String {
    let cleaned = strip_pasted_and_code_blocks(prompt);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return "New Task".to_string();
    }

    let path_words = extract_salient_path_or_repo_words(trimmed);

    // Focus on the first substantive sentence/clause for topic extraction.
    let no_parens = strip_parentheticals(trimmed);
    let first_clause = no_parens
        .split(['\n', '!', '?'])
        .flat_map(|line| line.split(". "))
        .map(str::trim)
        .find(|clause| clause.chars().any(|c| c.is_alphabetic()))
        .unwrap_or(no_parens.trim());

    let mut clause = first_clause.trim();
    loop {
        let lower = clause.to_ascii_lowercase();
        let mut matched = false;
        for prefix in CONVERSATIONAL_PREFIXES {
            if lower.starts_with(prefix) {
                clause = clause[prefix.len()..].trim();
                matched = true;
                break;
            }
        }
        if !matched {
            break;
        }
    }

    // Rewrite natural English "<head> of [the] <subject>" (e.g. "titles of the sessions" -> "Session Titles").
    let raw_words: Vec<&str> = clause
        .split_whitespace()
        .filter(|w| !w.starts_with("http://") && !w.starts_with("https://") && !w.contains('/'))
        .collect();
    for win_idx in 0..raw_words.len() {
        let head = raw_words[win_idx].trim_matches(|c: char| !c.is_alphanumeric());
        if head.is_empty() || STOP_WORDS.contains(&head.to_ascii_lowercase().as_str()) {
            continue;
        }
        if raw_words
            .get(win_idx + 1)
            .is_some_and(|w| w.eq_ignore_ascii_case("of"))
        {
            let subj_idx = if raw_words.get(win_idx + 2).is_some_and(|w| {
                matches!(
                    w.to_ascii_lowercase().as_str(),
                    "the" | "a" | "an" | "our" | "my" | "this"
                )
            }) {
                win_idx + 3
            } else {
                win_idx + 2
            };
            if let Some(subj_raw) = raw_words.get(subj_idx) {
                let subj = subj_raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
                if !subj.is_empty() && !STOP_WORDS.contains(&subj.to_ascii_lowercase().as_str()) {
                    let subj_singular = singularize_plural_head(subj);
                    let w1 = canonical_word_case(&subj_singular);
                    let w2 = canonical_word_case(head);
                    if !w1.is_empty() && !w2.is_empty() {
                        return format!("{w1} {w2}");
                    }
                }
            }
        }
    }

    // Collect non-stop topic words from the stripped clause.
    let mut topic_words: Vec<String> = Vec::new();
    for tok in clause.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let tok_clean = tok.trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '(' | ')' | '[' | ']'));
        if tok_clean.is_empty()
            || tok_clean.starts_with("http://")
            || tok_clean.starts_with("https://")
            || tok_clean.contains('/')
            || tok_clean.starts_with('~')
        {
            continue;
        }
        // Skip standalone ranges/numbers like "1-3".
        let trimmed_tok = tok_clean.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        if trimmed_tok.is_empty() || !trimmed_tok.chars().any(|c| c.is_alphabetic()) {
            continue;
        }
        if let Some((base, ext)) = trimmed_tok.rsplit_once('.') {
            if ext.len() <= 4 && BOILERPLATE_PATH_STEMS.contains(&base.to_ascii_lowercase().as_str()) {
                continue;
            }
        }
        for sub in trimmed_tok.split(['-', '_']) {
            let lower = sub.to_ascii_lowercase();
            if sub.len() <= 1
                || STOP_WORDS.contains(&lower.as_str())
                || BOILERPLATE_PATH_STEMS.contains(&lower.as_str())
            {
                continue;
            }
            let cased = canonical_word_case(sub);
            if !cased.is_empty()
                && !topic_words
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&cased))
            {
                topic_words.push(cased);
                if topic_words.len() >= 3 {
                    break;
                }
            }
        }
        if topic_words.len() >= 3 {
            break;
        }
    }

    // Combine repo/path subject with topic words intelligently (strictly 1..=3 words).
    if path_words.len() >= 2 && topic_words.is_empty() {
        return path_words.into_iter().take(3).collect::<Vec<_>>().join(" ");
    }
    if path_words.len() >= 2 && topic_words.len() <= 1 {
        let mut combined = path_words;
        for w in topic_words {
            if combined.len() < 3 && !combined.iter().any(|x| x.eq_ignore_ascii_case(&w)) {
                combined.push(w);
            }
        }
        return combined.into_iter().take(3).collect::<Vec<_>>().join(" ");
    }
    if !path_words.is_empty() && !topic_words.is_empty() {
        // Put concrete path/module anchor first if 1 word (e.g. "PTY Race Condition" -> "PTY Race Fix").
        let mut merged = Vec::with_capacity(3);
        for w in path_words.iter().take(2) {
            merged.push(w.clone());
        }
        for w in &topic_words {
            if merged.len() >= 3 {
                break;
            }
            if !merged.iter().any(|x| x.eq_ignore_ascii_case(w)) {
                merged.push(w.clone());
            }
        }
        return merged.join(" ");
    }
    if !path_words.is_empty() {
        return path_words.into_iter().take(3).collect::<Vec<_>>().join(" ");
    }
    if !topic_words.is_empty() {
        return topic_words.into_iter().take(3).collect::<Vec<_>>().join(" ");
    }

    // Last-resort fallback: first 1–3 alphabetic words in Title Case.
    let fallback: Vec<String> = first_clause
        .split_whitespace()
        .map(canonical_word_case)
        .filter(|w| !w.is_empty())
        .take(3)
        .collect();
    if fallback.is_empty() {
        "New Task".to_string()
    } else {
        fallback.join(" ")
    }
}

/// Normalizes raw LLM output into a clean 1–3 word title, or `None` if unusable.
pub fn normalize_concise_title(raw: &str) -> Option<String> {
    let first_line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("```"))?;

    let mut s = first_line.trim();
    // Strip markdown heading/bullet prefixes and "Title:" labels.
    s = s.trim_start_matches(['#', '-', '*', '>', ' ']);
    if let Some(rest) = s
        .strip_prefix("Title:")
        .or_else(|| s.strip_prefix("title:"))
        .or_else(|| s.strip_prefix("Session Title:"))
    {
        s = rest.trim();
    }
    // Strip surrounding emphasis or quotes.
    for wrapper in ["**", "__", "\"", "'", "`", "“", "”"] {
        if s.len() > wrapper.len() * 2 && s.starts_with(wrapper) && s.ends_with(wrapper) {
            s = s[wrapper.len()..s.len() - wrapper.len()].trim();
        }
    }
    s = s.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '`' | '*' | '_' | '.' | ':' | ';' | '!' | '?' | ',' | '-'
        )
    });
    if s.is_empty() {
        return None;
    }

    let mut words: Vec<String> = Vec::new();
    for raw_w in s.split_whitespace() {
        let cleaned = raw_w.trim_matches(|c: char| {
            matches!(c, '"' | '\'' | '`' | '*' | '_' | ',' | '.' | ':' | ';' | '(' | ')')
        });
        if cleaned.is_empty() {
            continue;
        }
        // Drop accidental conversational first word from weaker models.
        if words.is_empty()
            && matches!(
                cleaned.to_ascii_lowercase().as_str(),
                "please" | "session" | "conversation" | "chat"
            )
        {
            continue;
        }
        let formatted = canonical_word_case(cleaned);
        if !formatted.is_empty() {
            words.push(formatted);
        }
        if words.len() >= 3 {
            break;
        }
    }

    if words.is_empty() {
        return None;
    }
    let joined = words.join(" ");
    if joined.chars().count() > 36 {
        Some(words.into_iter().take(2).collect::<Vec<_>>().join(" "))
    } else {
        Some(joined)
    }
}

pub fn first_user_prompt_from_messages(messages: &[Value]) -> Option<String> {
    for msg in messages {
        if msg.get("type").and_then(Value::as_str) != Some("user") {
            continue;
        }
        if let Some(blocks) = msg.pointer("/message/content").and_then(Value::as_array) {
            let text: String = blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Some(s) = msg.pointer("/message/content").and_then(Value::as_str) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Detects legacy raw-prompt titles (e.g. 48/80-char prompt slices or default placeholders)
/// without ever false-positiving on short custom or user-renamed titles.
pub fn is_legacy_raw_prompt_title(title: &str, messages: &[Value]) -> bool {
    let trimmed = title.trim();
    let Some(first_prompt) = first_user_prompt_from_messages(messages) else {
        return false;
    };
    if trimmed.is_empty()
        || matches!(
            trimmed,
            "QuixiDesk Session" | "QuixiDesk Code Session" | "New task" | "Untitled task"
        )
    {
        return true;
    }

    let norm_title = collapse_ws(trimmed);
    let norm_prompt = collapse_ws(&first_prompt);
    if !norm_prompt.starts_with(&norm_title) {
        return false;
    }

    let word_count = norm_title.split_whitespace().count();
    let lower = norm_title.to_ascii_lowercase();
    word_count > 3
        || norm_title.chars().count() >= 28
        || norm_title.contains("://")
        || norm_title.contains('/')
        || lower.starts_with("please ")
        || lower.starts_with("can you ")
        || lower.starts_with("could you ")
        || lower.starts_with("help me ")
        || lower.starts_with("i want ")
        || lower.starts_with("make it ")
}

impl QuixiEngine {
    pub fn generate_llm_session_title(&self, prompt: &str, model_hint: Option<&str>) -> Option<String> {
        let cleaned = strip_pasted_and_code_blocks(prompt);
        let excerpt: String = cleaned.trim().chars().take(1_000).collect();
        if excerpt.is_empty() {
            return None;
        }
        let payload = json!({ "firstUserMessage": excerpt });
        let req_messages = vec![
            json!({
                "role": "system",
                "content": TITLE_SYSTEM_PROMPT,
            }),
            json!({
                "role": "user",
                "content": format!(
                    "Return a 1 to 3 word session title for this JSON payload:\n{}",
                    serde_json::to_string(&payload).unwrap_or_default()
                ),
            }),
        ];
        let turn = self
            .provider_manager
            .complete_turn_with_tools_for_provider_and_effort(
                &req_messages,
                &json!([]),
                model_hint.filter(|m| !m.trim().is_empty()),
                None,
                Some("off"),
                |_| {},
            )?;
        if turn.stream_error.is_some() || !turn.tool_calls.is_empty() {
            return None;
        }
        normalize_concise_title(&turn.text)
    }

    pub(crate) fn apply_session_title(&self, session_id: &str, new_title: String) -> Option<Value> {
        let trimmed = new_title.trim();
        if trimmed.is_empty() {
            return None;
        }
        let agent_update = {
            let mut map = self.agent_sessions.write();
            if let Some(sess) = map.get_mut(session_id) {
                if sess.title == trimmed {
                    return serde_json::to_value(sess.clone()).ok();
                }
                sess.title = trimmed.to_string();
                self.persist_agent_session(sess);
                Some(sess.clone())
            } else {
                None
            }
        };
        if let Some(snapshot) = agent_update {
            self.emit_eipc(
                "quixi.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "session_updated",
                    "sessionId": session_id,
                    "session": snapshot,
                }),
            );
            return serde_json::to_value(snapshot).ok();
        }

        let code_update = {
            let mut map = self.code_sessions.write();
            if let Some(sess) = map.get_mut(session_id) {
                if sess.title == trimmed {
                    return serde_json::to_value(sess.clone()).ok();
                }
                sess.title = trimmed.to_string();
                self.persist_code_session(sess);
                Some(sess.clone())
            } else {
                None
            }
        };
        if let Some(snapshot) = code_update {
            self.emit_eipc(
                "quixi.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "session_updated",
                    "sessionId": session_id,
                    "session": snapshot,
                }),
            );
            return serde_json::to_value(snapshot).ok();
        }
        None
    }

    /// Sets an immediate 1–3 word fallback title on the first prompt (when unpinned)
    /// and asynchronously upgrades it via the connected LLM in production builds.
    pub fn auto_title_session_on_first_prompt(self: &Arc<Self>, session_id: &str, prompt: &str) {
        let trimmed = prompt.trim();
        if trimmed.is_empty() || self.collaboration.is_title_pinned(session_id) {
            return;
        }
        let is_fork = self
            .agent_sessions
            .read()
            .get(session_id)
            .is_some_and(|s| s.forked_from.is_some())
            || self
                .code_sessions
                .read()
                .get(session_id)
                .is_some_and(|s| s.forked_from.is_some());
        if is_fork {
            return;
        }

        let fallback = smart_fallback_title(trimmed);
        self.apply_session_title(session_id, fallback);

        if cfg!(test) {
            return;
        }

        let model_hint = self
            .agent_sessions
            .read()
            .get(session_id)
            .map(|s| s.model.clone())
            .or_else(|| self.code_sessions.read().get(session_id).map(|s| s.model.clone()));
        let eng = Arc::clone(self);
        let sid = session_id.to_string();
        let prompt_owned = trimmed.to_string();
        std::thread::spawn(move || {
            if eng.collaboration.is_title_pinned(&sid) {
                return;
            }
            if let Some(llm_title) =
                eng.generate_llm_session_title(&prompt_owned, model_hint.as_deref())
            {
                if !eng.collaboration.is_title_pinned(&sid) {
                    eng.collaboration.set_title_generated(&sid, true);
                    eng.apply_session_title(&sid, llm_title);
                }
            }
        });
    }

    /// Explicit user-requested regeneration from the session menu ("Generate title").
    pub fn refresh_session_title(self: &Arc<Self>, session_id: &str) -> Value {
        let (prompt_opt, model_hint) = if let Some(sess) = self.agent_sessions.read().get(session_id) {
            (
                first_user_prompt_from_messages(&sess.buffered_messages),
                Some(sess.model.clone()),
            )
        } else if let Some(sess) = self.code_sessions.read().get(session_id) {
            let transcript = self
                .code_transcripts
                .read()
                .get(session_id)
                .cloned()
                .unwrap_or_default();
            (
                first_user_prompt_from_messages(&transcript),
                Some(sess.model.clone()),
            )
        } else {
            return json!({"error": "Session not found"});
        };

        let Some(prompt) = prompt_opt else {
            return json!({"error": "Send a message before generating a title."});
        };

        self.collaboration.set_title_pinned(session_id, false);
        let title = self
            .generate_llm_session_title(&prompt, model_hint.as_deref())
            .unwrap_or_else(|| smart_fallback_title(&prompt));
        self.collaboration.set_title_generated(session_id, true);
        self.apply_session_title(session_id, title.clone())
            .unwrap_or_else(|| json!({"sessionId": session_id, "title": title}))
    }

    /// Upgrades existing raw-prompt session titles loaded from disk on startup.
    pub(crate) fn upgrade_legacy_session_titles(self: &Arc<Self>) {
        let mut candidates: Vec<(String, String, Option<String>)> = Vec::new();

        {
            let mut map = self.agent_sessions.write();
            for sess in map.values_mut() {
                if sess.forked_from.is_some()
                    || self.collaboration.is_title_pinned(&sess.session_id)
                    || self.collaboration.is_title_generated(&sess.session_id)
                {
                    continue;
                }
                if is_legacy_raw_prompt_title(&sess.title, &sess.buffered_messages) {
                    if let Some(first_prompt) =
                        first_user_prompt_from_messages(&sess.buffered_messages)
                    {
                        let fallback = smart_fallback_title(&first_prompt);
                        if sess.title != fallback {
                            sess.title = fallback;
                            self.persist_agent_session(sess);
                        }
                        candidates.push((
                            sess.session_id.clone(),
                            first_prompt,
                            Some(sess.model.clone()),
                        ));
                    }
                }
            }
        }

        {
            let transcripts = self.code_transcripts.read();
            let mut map = self.code_sessions.write();
            for sess in map.values_mut() {
                if sess.forked_from.is_some()
                    || self.collaboration.is_title_pinned(&sess.session_id)
                    || self.collaboration.is_title_generated(&sess.session_id)
                {
                    continue;
                }
                let tr = transcripts
                    .get(&sess.session_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                if is_legacy_raw_prompt_title(&sess.title, tr) {
                    if let Some(first_prompt) = first_user_prompt_from_messages(tr) {
                        let fallback = smart_fallback_title(&first_prompt);
                        if sess.title != fallback {
                            sess.title = fallback;
                            self.persist_code_session(sess);
                        }
                        candidates.push((
                            sess.session_id.clone(),
                            first_prompt,
                            Some(sess.model.clone()),
                        ));
                    }
                }
            }
        }

        if cfg!(test) || candidates.is_empty() {
            return;
        }

        let eng = Arc::clone(self);
        std::thread::spawn(move || {
            for (sid, first_prompt, model_hint) in candidates.into_iter().take(25) {
                if eng.collaboration.is_title_pinned(&sid)
                    || eng.collaboration.is_title_generated(&sid)
                {
                    continue;
                }
                if let Some(llm_title) =
                    eng.generate_llm_session_title(&first_prompt, model_hint.as_deref())
                {
                    if !eng.collaboration.is_title_pinned(&sid) {
                        eng.collaboration.set_title_generated(&sid, true);
                        eng.apply_session_title(&sid, llm_title);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn smart_fallback_title_extracts_crisp_one_to_three_word_subjects() {
        let t1 = smart_fallback_title(
            "Please read handoff.md and continue our work on docs/proposals/bridge-the-impl-gap.md",
        );
        assert_eq!(t1, "Bridge Impl Gap");

        let t2 = smart_fallback_title("Please explore https://github.com/deepseek-ai/deepseek-harness");
        assert_eq!(t2, "DeepSeek Harness");

        let t3 = smart_fallback_title("make it easier to debug long turns when tool calls drift");
        assert_eq!(t3, "Debug Long Turns");

        let t4 = smart_fallback_title(
            "the titles of the sessions (on the left) are not good. it should be like, 1-3 words and really nailing what it's about",
        );
        assert_eq!(t4, "Session Titles");

        let t5 = smart_fallback_title("fix race condition in crates/quixi-workspace/src/pty.rs");
        assert!(t5.split_whitespace().count() <= 3, "got {t5}");
        assert!(t5.contains("PTY"), "got {t5}");
    }

    #[test]
    fn normalize_concise_title_strips_wrappers_and_caps_at_three_words() {
        assert_eq!(
            normalize_concise_title("**DeepSeek Harness Upgrade**."),
            Some("DeepSeek Harness Upgrade".into())
        );
        assert_eq!(
            normalize_concise_title("Title: \"Sidebar Session Titles\""),
            Some("Sidebar Session Titles".into())
        );
        assert_eq!(
            normalize_concise_title("Fix macOS Seatbelt Sandbox Policy Denied"),
            Some("Fix macOS Seatbelt".into())
        );
        assert_eq!(normalize_concise_title("   \n```json\n"), None);
    }

    #[test]
    fn legacy_raw_prompt_detector_catches_truncated_prompts_without_touching_custom_titles() {
        let msgs = vec![json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{"type": "text", "text": "Please read handoff.md and continue our work on docs/proposals/bridge-the-impl-gap.md"}]
            }
        })];
        assert!(is_legacy_raw_prompt_title(
            "Please read handoff.md and continue our work on ",
            &msgs
        ));
        assert!(!is_legacy_raw_prompt_title("Bridge Impl Gap", &msgs));
        assert!(!is_legacy_raw_prompt_title("latest unsaved title", &msgs));
        assert!(!is_legacy_raw_prompt_title("Please read handoff.md", &[]));
    }

    #[test]
    fn llm_session_title_refresh_and_manual_rename_pinning_work() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock llm");
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                let _ = sock.read(&mut buf);
                let sse = concat!(
                    "data: {\"choices\":[{\"delta\":{\"content\":\"**Sidebar Titles**\"}}]}\n\n",
                    "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                    "data: [DONE]\n\n"
                );
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    sse.len(),
                    sse
                );
                let _ = sock.write_all(resp.as_bytes());
            }
        });

        let dir = tempfile::tempdir().unwrap();
        let engine = QuixiEngine::new_with_data_dir(dir.path().join("res"), dir.path().join("data"));
        {
            let mut st = engine.provider_manager.state.write();
            st.active_provider = "custom".into();
            st.active_model = "title-model".into();
            let entry = st.providers.iter_mut().find(|p| p.id == "custom").unwrap();
            entry.configured = true;
            entry.auth_mode = "api_key".into();
            entry.api_key_raw = "sk-fixture".into();
            entry.api_format = "openai".into();
            entry.base_url = format!("http://{addr}/v1");
            entry.selected_model = "title-model".into();
        }

        let sid = engine.start_agent_session(&json!({
            "autoTitle": true,
            "title": "New Task",
            "model": "title-model"
        }));
        assert!(!engine.collaboration.is_title_pinned(&sid));

        // Seed a user prompt and run explicit refreshSessionTitle -> hits mock LLM and normalizes to "Sidebar Titles".
        {
            let mut map = engine.agent_sessions.write();
            let sess = map.get_mut(&sid).unwrap();
            sess.buffered_messages.push(json!({
                "type": "user",
                "message": {"role": "user", "content": [{"type": "text", "text": "the titles of the sessions on the left are not good"}]}
            }));
            engine.persist_agent_session(sess);
        }
        let refreshed = engine.refresh_session_title(&sid);
        assert_eq!(refreshed["title"], "Sidebar Titles");
        assert!(engine.collaboration.is_title_generated(&sid));

        // Pinning via manual rename prevents auto_title_session_on_first_prompt from overwriting.
        engine.collaboration.set_title_pinned(&sid, true);
        engine.apply_session_title(&sid, "Custom Title".into());
        engine.auto_title_session_on_first_prompt(&sid, "Please explore https://github.com/org/other-repo");
        assert_eq!(engine.agent_sessions.read()[&sid].title, "Custom Title");
    }
}
