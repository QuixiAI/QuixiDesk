//! Stress and edge-case tests for the skill subsystem.
//!
//! Everything here builds its own fixture tree in a temp directory and points
//! `QUIXI_SKILL_ROOTS` at it, so a run never reads the real user data or skill roots
//! skill trees. That matters twice over: the assertions stay deterministic on any machine, and
//! the suite is safe to run inside a container with no home directory at all.
//!
//! `QUIXI_SKILL_ROOTS` is a process-wide variable, so tests take `GUARD` and run one at a time.
//! Run with `--test-threads=1` if that ever becomes a problem.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;

use quixi_engine::QuixiEngine;

static GUARD: Mutex<()> = Mutex::new(());

/// One failing test must not poison the lock and fail every test after it.
fn guard() -> std::sync::MutexGuard<'static, ()> {
    GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "quixi-skills-stress-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("skills")).unwrap();
        std::fs::create_dir_all(root.join("data")).unwrap();
        // An engine constructed with an explicit root must not also scan the default roots.
        std::env::set_var("QUIXI_SKILL_ROOTS", root.join("skills"));
        Self { root }
    }

    fn skills_dir(&self) -> PathBuf {
        self.root.join("skills")
    }

    /// Write `<rel>/SKILL.md`.
    fn skill(&self, rel: &str, body: &str) -> PathBuf {
        let dir = self.skills_dir().join(rel);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), body).unwrap();
        dir
    }

    fn engine(&self) -> Arc<QuixiEngine> {
        QuixiEngine::new_with_data_dir(
            self.root.join("resources"),
            self.root.join("data"),
        )
    }

    fn ids(&self) -> Vec<String> {
        self.engine()
            .list_local_skills()
            .iter()
            .filter_map(|s| s.get("skillId").and_then(|v| v.as_str()).map(string))
            .collect()
    }
}

fn string(s: &str) -> String {
    s.to_string()
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn frontmatter(name: &str, desc: &str) -> String {
    format!("---\nname: {name}\ndescription: {desc}\nlicense: MIT\n---\n\n# {name}\n\nBody text.\n")
}

fn path_of(p: &Path) -> String {
    p.display().to_string()
}

// ── discovery ──────────────────────────────────────────────────────────────

#[test]
fn an_empty_tree_yields_no_skills_and_does_not_panic() {
    let _g = guard();
    let f = Fixture::new("empty");
    assert!(f.ids().is_empty());

    let (out, is_err) = f.engine().list_skills_tool(&json!({}));
    assert!(is_err, "an empty library should report as an error state: {out}");
}

#[test]
fn a_skill_two_levels_deep_is_found_nested_by_area() {
    let _g = guard();
    let f = Fixture::new("nested");
    f.skill("devops/stealth-browser", &frontmatter("stealth-browser", "Drive a browser"));

    let ids = f.ids();
    assert_eq!(ids, vec!["devops/stealth-browser".to_string()]);
}

#[test]
fn a_flat_skill_keeps_its_bare_id() {
    let _g = guard();
    let f = Fixture::new("flat");
    f.skill("browser-driving", &frontmatter("browser-driving", "Drive a browser"));

    assert_eq!(f.ids(), vec!["browser-driving".to_string()]);
}

#[test]
fn dot_directories_are_skipped_so_archived_skills_do_not_shadow_live_ones() {
    let _g = guard();
    let f = Fixture::new("dotdir");
    f.skill("live-skill", &frontmatter("live-skill", "Current version"));
    f.skill(".archive/live-skill-20260101", &frontmatter("live-skill", "Old version"));
    f.skill("target/debug/thing", &frontmatter("thing", "Build output"));

    let ids = f.ids();
    assert_eq!(ids, vec!["live-skill".to_string()], "got {ids:?}");
}

#[test]
fn reference_files_beside_a_skill_are_not_treated_as_skills() {
    let _g = guard();
    let f = Fixture::new("refs");
    let dir = f.skill("pdf", &frontmatter("pdf", "Make PDFs"));
    std::fs::create_dir_all(dir.join("references")).unwrap();
    std::fs::write(dir.join("references/notes.md"), "# notes\n").unwrap();
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::write(dir.join("scripts/run.sh"), "#!/bin/sh\n").unwrap();

    assert_eq!(f.ids(), vec!["pdf".to_string()]);
}

// ── description extraction ─────────────────────────────────────────────────

#[test]
fn a_missing_frontmatter_falls_back_to_the_first_prose_line() {
    let _g = guard();
    let f = Fixture::new("nofm");
    f.skill("bare", "# A heading\n\nThe real first line.\n\nMore.\n");

    let skills = f.engine().list_local_skills();
    let desc = skills[0]["description"].as_str().unwrap();
    assert_eq!(desc, "The real first line.");
}

#[test]
fn a_file_that_is_only_a_heading_gets_the_default_description() {
    let _g = guard();
    let f = Fixture::new("onlyheading");
    f.skill("empty-ish", "# Nothing else here\n");

    let skills = f.engine().list_local_skills();
    let desc = skills[0]["description"].as_str().unwrap();
    assert_eq!(desc, "Custom QuixiDesk skill");
}

#[test]
fn a_binary_file_does_not_panic_the_scanner() {
    let _g = guard();
    let f = Fixture::new("binary");
    let dir = f.skills_dir().join("garbage");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), [0u8, 159, 146, 150, 255, 0, 12]).unwrap();

    let ids = f.ids();
    assert_eq!(ids, vec!["garbage".to_string()], "scanner must survive: {ids:?}");
}

#[test]
fn duplicate_names_in_different_areas_stay_distinct() {
    let _g = guard();
    let f = Fixture::new("dupes");
    f.skill("identity/example-personality", &frontmatter("example-personality", "Identity one"));
    f.skill("archive-two/example-personality", &frontmatter("example-personality", "Identity two"));

    let ids = f.ids();
    assert_eq!(ids.len(), 2, "both must be listed: {ids:?}");
    assert!(ids.iter().all(|i| i.contains('/')));
}

// ── loading ────────────────────────────────────────────────────────────────

#[test]
fn load_skill_returns_the_body_and_the_directory() {
    let _g = guard();
    let f = Fixture::new("load");
    let dir = f.skill("pdf", &frontmatter("pdf", "Make PDFs"));

    let (out, is_err) = f.engine().load_skill(&json!({"skillId": "pdf"}));
    assert!(!is_err, "{out}");
    assert!(out.contains("Skill directory:"), "{out}");
    assert!(out.contains(&path_of(&dir)), "must name the real directory: {out}");
    assert!(out.contains("Body text."), "must include the body: {out}");
}

#[test]
fn traversal_attempts_are_refused() {
    let _g = guard();
    let f = Fixture::new("traversal");
    f.skill("pdf", &frontmatter("pdf", "Make PDFs"));
    let engine = f.engine();

    for bad in ["../../etc/passwd", "/etc/passwd", "..", ".", "a/../../b", "dir\\win"] {
        let (out, is_err) = engine.load_skill(&json!({"skillId": bad}));
        assert!(is_err, "`{bad}` must be refused, got: {out}");
        assert!(!out.contains("root:"), "leaked a file for `{bad}`: {out}");
    }
}

#[test]
fn a_missing_skill_lists_what_is_available() {
    let _g = guard();
    let f = Fixture::new("missing");
    f.skill("pdf", &frontmatter("pdf", "Make PDFs"));

    let (out, is_err) = f.engine().load_skill(&json!({"skillId": "nope"}));
    assert!(is_err);
    assert!(out.contains("pdf"), "should suggest what exists: {out}");
}

#[test]
fn an_oversized_body_is_truncated_rather_than_dumped() {
    let _g = guard();
    let f = Fixture::new("huge");
    let body = format!("{}\n", "x".repeat(200_000));
    f.skill("huge", &body);

    let (out, is_err) = f.engine().load_skill(&json!({"skillId": "huge"}));
    assert!(!is_err);
    assert!(out.len() < 70_000, "body must be capped, got {} bytes", out.len());
}

// ── pagination and search ──────────────────────────────────────────────────

#[test]
fn a_large_library_pages_fifty_at_a_time() {
    let _g = guard();
    let f = Fixture::new("paging");
    for i in 0..120 {
        f.skill(&format!("bulk/skill-{i:03}"), &frontmatter(&format!("skill-{i:03}"), "bulk"));
    }

    let engine = f.engine();
    let (p1, _) = engine.list_skills_tool(&json!({}));
    assert!(p1.contains("page 1 of 3"), "{p1}");
    let listed = p1.lines().filter(|l| l.starts_with("bulk/")).count();
    assert_eq!(listed, 50, "must show exactly 50: {listed}");

    let (p3, _) = engine.list_skills_tool(&json!({"page": 3}));
    assert!(p3.contains("page 3 of 3"), "{p3}");
    assert!(p3.contains("End of results"), "{p3}");
}

#[test]
fn a_page_past_the_end_clamps_instead_of_returning_nothing() {
    let _g = guard();
    let f = Fixture::new("clamp");
    f.skill("only", &frontmatter("only", "just one"));

    let (out, _) = f.engine().list_skills_tool(&json!({"page": 9999}));
    assert!(out.contains("page 1 of 1"), "{out}");
    assert!(out.contains("only"), "{out}");
}

#[test]
fn page_zero_and_negative_style_inputs_do_not_underflow() {
    let _g = guard();
    let f = Fixture::new("pagezero");
    for i in 0..3 {
        f.skill(&format!("s{i}"), &frontmatter(&format!("s{i}"), "d"));
    }
    let engine = f.engine();

    let (out, _) = engine.list_skills_tool(&json!({"page": 0}));
    assert!(out.contains("page 1 of 1"), "{out}");
}

#[test]
fn search_ranks_a_name_match_above_a_description_mention() {
    let _g = guard();
    let f = Fixture::new("rank");
    f.skill("pdf", &frontmatter("pdf", "Create PDFs"));
    f.skill("other/tangent", &frontmatter("tangent", "Mentions pdf in passing only"));

    let (out, _) = f.engine().list_skills_tool(&json!({"query": "pdf"}));
    let first = out.lines().nth(1).unwrap_or("");
    assert!(first.starts_with("pdf:"), "exact id must rank first, got: {out}");
}

#[test]
fn an_unmatched_query_says_so_and_suggests_breadth() {
    let _g = guard();
    let f = Fixture::new("nomatch");
    f.skill("pdf", &frontmatter("pdf", "Create PDFs"));

    let (out, is_err) = f.engine().list_skills_tool(&json!({"query": "zzzznotathing"}));
    assert!(is_err);
    assert!(out.contains("1 skills are installed"), "{out}");
}

#[test]
fn a_very_long_query_does_not_match_everything() {
    let _g = guard();
    let f = Fixture::new("longquery");
    f.skill("pdf", &frontmatter("pdf", "Create PDFs"));

    let long = "a".repeat(5_000);
    let (_, is_err) = f.engine().list_skills_tool(&json!({"query": long}));
    assert!(is_err, "a 5k query should match nothing");
}

#[test]
fn unicode_and_spaced_names_survive_the_round_trip() {
    let _g = guard();
    let f = Fixture::new("unicode");
    f.skill("café/ménage", &frontmatter("ménage", "Unicode id"));

    let ids = f.ids();
    assert_eq!(ids.len(), 1, "{ids:?}");
    let id = &ids[0];
    let (out, is_err) = f.engine().load_skill(&json!({"skillId": id}));
    assert!(!is_err, "must load a unicode id: {out}");
}
