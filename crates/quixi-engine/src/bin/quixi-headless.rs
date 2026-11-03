//! quixi-headless — headless driver for the QuixiDesk engine.
//!
//! The desktop app and this binary run the *same* engine; the only difference is that this one
//! never constructs a window. It links no GUI crate at all, so it runs wherever the engine runs:
//! a server, a container, an SSH session, macOS, or a terminal on a phone.
//!
//! What it gives up is ComputerUse. Screenshots, synthetic input and window management need a
//! display to talk to; on a box with none there is nothing to capture and nothing to click. Chat,
//! tools, files, MCP, subagents and skills all work.
//!
//!   quixi-headless "explain this repo"          one-shot prompt, streamed to stdout
//!   quixi-headless --skills                     list the skills the model is told about
//!   quixi-headless --skill pdf                  print one skill's body (what LoadSkill returns)
//!   quixi-headless --providers                  show configured providers and the active model
//!   quixi-headless --cwd /some/where "prompt"   run the agent in a specific working directory
//!   quixi-headless --json "prompt"              emit the final reply as JSON instead of streaming

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use quixi_engine::providers::StreamChunk;
use quixi_engine::QuixiEngine;

/// Find the resources directory the engine boots from.
///
/// Mirrors the desktop app's lookup so both find the same bootstrap files, with the installed
/// Arch layout as a last resort.
fn locate_resources() -> PathBuf {
    if let Ok(env_dir) = std::env::var("QUIXI_RESOURCES_DIR") {
        let p = PathBuf::from(env_dir);
        if p.exists() {
            return p;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // portable, cargo target/release, and macOS .app bundle layouts
            for candidate in [
                dir.join("resources"),
                dir.join("../../resources"),
                dir.join("../Resources"),
            ] {
                if candidate.join("bootstrap").exists() || candidate.join("ui/index.html").exists() {
                    return candidate.canonicalize().unwrap_or(candidate);
                }
            }
        }
    }

    // Linux distribution package layout (e.g. the Arch package installs to /usr/lib/quixidesk).
    PathBuf::from("/usr/lib/quixidesk/resources")
}

fn usage_with(code: i32) -> ! {
    eprintln!(
        "quixi-headless — headless QuixiDesk engine\n\
         \n\
         usage:\n\
         \x20 quixi-headless \"prompt\"              run one prompt, streamed\n\
         \x20 quixi-headless --skills             list skills available to the model\n\
         \x20 quixi-headless --skill <id>         print one skill body\n\
         \x20 quixi-headless --providers          list providers\n\
         \x20 quixi-headless --capabilities       report what this session can actually do\n\
         \x20 quixi-headless --cwd <dir> \"prompt\"  set the working directory\n\
         \x20 quixi-headless --json \"prompt\"       print the reply as JSON\n\
         \n\
         environment:\n\
         \x20 QUIXI_RESOURCES_DIR   override the resources directory\n\
         \x20 QUIXI_DATA_DIR        override the data directory (default ~/.quixi)"
    );
    std::process::exit(code)
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv[0] == "--help" || argv[0] == "-h" {
        usage_with(if argv.is_empty() { 2 } else { 0 });
    }
    // An unrecognised leading option is a mistake, not a prompt: sending `--bogus` to the model
    // spends a call answering a question the user did not ask, and looks like the CLI hung.
    const KNOWN: [&str; 12] = [
        "--skills", "--skill", "--search", "--providers", "--prompt-block", "--json", "--cwd",
        "--capabilities", "--mode", "--plan", "--model", "--effort",
    ];
    if argv[0].starts_with('-') && !KNOWN.contains(&argv[0].as_str()) {
        eprintln!("quixi-headless: unknown option `{}`\n", argv[0]);
        usage_with(2);
    }

    let resources = locate_resources();
    let engine = QuixiEngine::new(resources);

    // ── inspection commands: no model call, no cost ──
    // What this session can actually do — the same probe the UI uses to decide which
    // capabilities to hide. First thing to ask for in a bug report: it separates
    // "the capability is missing here" from "the backend failed".
    if argv[0] == "--capabilities" {
        #[cfg(target_os = "linux")]
        let capabilities = serde_json::to_value(quixi_platform::linux::capabilities())
            .unwrap_or_else(|e| serde_json::json!({"error": format!("capability probe failed: {e}")}));
        #[cfg(not(target_os = "linux"))]
        let capabilities = serde_json::json!({
            "session_type": std::env::consts::OS,
            "notes": ["The detailed capability probe is Linux-only; on this OS the native platform layer is used."]
        });
        match serde_json::to_string_pretty(&capabilities) {
            Ok(rendered) => println!("{rendered}"),
            Err(e) => {
                eprintln!("quixi-headless: could not render capabilities: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if argv[0] == "--skills" {
        let skills = engine.list_local_skills();
        if skills.is_empty() {
            println!("no skills installed");
        }
        for s in skills {
            println!(
                "{}: {}",
                s.get("skillId").and_then(|v| v.as_str()).unwrap_or("?"),
                s.get("description").and_then(|v| v.as_str()).unwrap_or("")
            );
        }
        return;
    }

    if argv[0] == "--skill" {
        let Some(id) = argv.get(1) else {
            eprintln!("--skill needs an id; try --skills");
            std::process::exit(2);
        };
        match engine.get_local_skill_files(id) {
            files if files.is_empty() => {
                eprintln!("no skill named {id}");
                std::process::exit(1);
            }
            files => {
                for f in files {
                    if let Some(body) = f.get("content").and_then(|v| v.as_str()) {
                        print!("{body}");
                    } else if let (Some(name), Some(path)) = (
                        f.get("name").and_then(|v| v.as_str()),
                        f.get("path").and_then(|v| v.as_str()),
                    ) {
                        println!("── {name} ({path})");
                    }
                }
            }
        }
        return;
    }

    if argv[0] == "--search" {
        // Same code path the model hits through the ListSkills tool, so this is a real check of
        // ranking and pagination rather than a mock of it.
        let query = argv
            .get(1)
            .filter(|a| !a.starts_with("--"))
            .cloned()
            .unwrap_or_default();
        let page = argv
            .iter()
            .position(|a| a == "--page")
            .and_then(|i| argv.get(i + 1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1);
        let (out, _is_error) =
            engine.list_skills_tool(&serde_json::json!({"query": query, "page": page}));
        println!("{out}");
        return;
    }

    if argv[0] == "--prompt-block" {
        // Exactly what the model is told about the skill library, and what it costs to build it.
        // Useful when the library grows and the per-turn overhead needs checking.
        let t0 = std::time::Instant::now();
        let block = engine.skills_index_block();
        let elapsed = t0.elapsed();
        println!("{}", block.trim());
        eprintln!("[cost] built in {:.1} ms", elapsed.as_secs_f64() * 1000.0);
        return;
    }

    if argv[0] == "--providers" {
        let st = engine.provider_manager.state.read();
        println!("active provider: {}", st.active_provider);
        println!("active model:    {}", st.active_model);
        if st.subagent_model_user_selected && !st.subagent_model.is_empty() {
            println!("subagent:        {} / {}", st.subagent_provider, st.subagent_model);
        } else {
            println!("subagent:        same as active model");
        }
        for p in st.providers.iter() {
            println!(
                "  {:<12} {:<28} {}",
                p.id,
                p.base_url,
                if p.configured { "configured" } else { "not configured" }
            );
        }
        return;
    }

    // ── a real turn ──
    let mut json_out = false;
    let mut plan_mode = false;
    let mut perm_mode = "default".to_string();
    let mut model_opt: Option<String> = None;
    let mut effort_opt = "default".to_string();
    let mut cwd = std::env::current_dir()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string());
    let mut prompt_parts: Vec<String> = Vec::new();

    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--json" => json_out = true,
            "--plan" => plan_mode = true,
            "--cwd" => {
                i += 1;
                if i < argv.len() {
                    cwd = argv[i].clone();
                }
            }
            "--mode" => {
                i += 1;
                if i < argv.len() {
                    perm_mode = argv[i].clone();
                }
            }
            "--model" => {
                i += 1;
                if i < argv.len() {
                    model_opt = Some(argv[i].clone());
                }
            }
            "--effort" => {
                i += 1;
                if i < argv.len() {
                    effort_opt = argv[i].clone();
                }
            }
            other => prompt_parts.push(other.to_string()),
        }
        i += 1;
    }

    let prompt = prompt_parts.join(" ");
    if prompt.trim().is_empty() {
        usage_with(2);
    }

    let session_id = engine.start_agent_session(&serde_json::json!({
        "cwd": cwd,
        "userSelectedFolders": [cwd],
        "permissionMode": perm_mode,
        "effortLevel": effort_opt,
        "model": model_opt.clone().unwrap_or_default(),
    }));
    if plan_mode {
        engine.collaboration.set_plan_mode(&session_id, true);
    }
    let cancelled = AtomicBool::new(false);

    let mut printed_any = false;
    let (reply, model) = engine.generate_reply_with_tools_streaming(
        &prompt,
        &cwd,
        &[],
        model_opt.as_deref(),
        &perm_mode,
        &session_id,
        &cancelled,
        |chunk| match chunk {
            // Streaming output goes to stdout so it can be piped; reasoning goes to stderr so it
            // does not contaminate the answer.
            StreamChunk::Text(t) => {
                if !json_out {
                    print!("{t}");
                    let _ = std::io::stdout().flush();
                    printed_any = true;
                }
            }
            StreamChunk::Thinking(t) => {
                if !json_out {
                    eprint!("{t}");
                }
            }
        },
        |envelope| {
            // Tool activity goes to stderr so the answer streams cleanly on stdout. The engine
            // hands back a message envelope whose content is a list of blocks, not OpenAI-style
            // tool_calls - reading the wrong path is why this printed nothing.
            if json_out {
                return;
            }
            let Some(blocks) = envelope
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
            else {
                return;
            };
            for block in blocks {
                if block.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                    continue;
                }
                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let detail = block
                    .get("input")
                    .map(|i| {
                        let s = i.to_string();
                        if s.chars().count() > 90 {
                            format!("{}…", s.chars().take(90).collect::<String>())
                        } else {
                            s
                        }
                    })
                    .unwrap_or_default();
                eprintln!("[tool] {name} {detail}");
            }
        },
    );

    if json_out {
        let collab = engine.collaboration.client_view(&session_id);
        println!(
            "{}",
            serde_json::json!({
                "model": model,
                "session": session_id,
                "reply": reply,
                "contextMeter": collab["contextMeter"],
                "deliverables": collab["deliverables"],
            })
        );
    } else if !printed_any {
        // Some providers return the whole answer without streaming; print it rather than nothing.
        println!("{reply}");
    } else {
        println!();
    }
}
