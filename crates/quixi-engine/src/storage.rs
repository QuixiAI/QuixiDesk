use super::*;
use parking_lot::Mutex;
use std::io::Write;
use std::sync::atomic::AtomicU64;

#[derive(Default)]
pub(crate) struct SessionStorage {
    writer: Mutex<()>,
    revision: AtomicU64,
    issues: RwLock<HashMap<String, Value>>,
}

// Never truncate the previous valid record. A failed write/rename leaves it intact.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

impl QuixiEngine {
    fn persist_session_document(&self, kind: &str, id: &str, title: &str, mut document: Value) {
        if id.trim().is_empty() {
            return;
        }
        let key = format!("{kind}/{id}");
        let _writer = self.storage.writer.lock();
        let clock = Utc::now().timestamp_nanos_opt().unwrap_or_default().max(0) as u64;
        let revision = self
            .storage
            .revision
            .fetch_max(clock, Ordering::SeqCst)
            .max(clock)
            + 1;
        self.storage.revision.store(revision, Ordering::SeqCst);
        document["_quixiPersistenceRevision"] = json!(revision);
        let raw = serde_json::to_vec_pretty(&document).expect("JSON value serializes");
        let primary = self.data_dir.join(kind).join(format!("{id}.json"));
        let recovery = self
            .data_dir
            .join("recovery")
            .join(kind)
            .join(format!("{id}.json"));
        let issue = match atomic_write(&primary, &raw) {
            Ok(()) => {
                let _ = fs::remove_file(&recovery);
                None
            }
            Err(error) => {
                let recovery_saved = atomic_write(&recovery, &raw).is_ok();
                Some(json!({"kind":kind,"sessionId":id,"title":title,
                    "message":format!("Cannot save conversation: {error}"),
                    "recoverySaved":recovery_saved}))
            }
        };
        let mut issues = self.storage.issues.write();
        let changed = if let Some(issue) = issue {
            if issues.get(&key) == Some(&issue) {
                false
            } else {
                issues.insert(key, issue);
                true
            }
        } else {
            issues.remove(&key).is_some()
        };
        drop(issues);
        drop(_writer);
        if changed {
            self.emit("quixi:storage-status", self.session_storage_status());
        }
    }

    pub fn persist_agent_session(&self, session: &LocalAgentSession) {
        self.persist_session_document(
            "sessions",
            &session.session_id,
            &session.title,
            json!(session),
        );
    }

    pub fn persist_code_session(&self, session: &LocalCodeSession) {
        let mut document = json!(session);
        document["transcript"] = json!(self
            .code_transcripts
            .read()
            .get(&session.session_id)
            .cloned()
            .unwrap_or_default());
        self.persist_session_document(
            "code_sessions",
            &session.session_id,
            &session.title,
            document,
        );
    }

    pub fn session_storage_status(&self) -> Value {
        let mut issues: Vec<Value> = self.storage.issues.read().values().cloned().collect();
        issues.sort_by_key(|value| value["sessionId"].as_str().unwrap_or_default().to_string());
        json!({"issues":issues})
    }

    pub fn has_unsaved_conversations(&self) -> bool {
        self.storage
            .issues
            .read()
            .values()
            .any(|issue| issue["recoverySaved"] != true)
    }

    pub fn session_storage_export(&self) -> Value {
        let pending = self
            .storage
            .issues
            .read()
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let sessions: Vec<_> = self
            .agent_sessions
            .read()
            .values()
            .filter(|session| pending.contains(&format!("sessions/{}", session.session_id)))
            .cloned()
            .collect();
        let code_sessions: Vec<_> = self
            .code_sessions
            .read()
            .values()
            .filter(|session| pending.contains(&format!("code_sessions/{}", session.session_id)))
            .cloned()
            .collect();
        let transcripts: HashMap<_, _> = self
            .code_transcripts
            .read()
            .iter()
            .filter(|(id, _)| pending.contains(&format!("code_sessions/{id}")))
            .map(|(id, messages)| (id.clone(), messages.clone()))
            .collect();
        json!({"sessions": sessions, "codeSessions": code_sessions, "codeTranscripts": transcripts})
    }

    pub fn retry_session_saves(&self) -> Value {
        let pending = self
            .storage
            .issues
            .read()
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        // Keep the session read lock until persistence finishes so deletion cannot race
        // a retry snapshot and resurrect a deleted conversation.
        for session in self.agent_sessions.read().values() {
            if pending.contains(&format!("sessions/{}", session.session_id)) {
                self.persist_agent_session(session);
            }
        }
        for session in self.code_sessions.read().values() {
            if pending.contains(&format!("code_sessions/{}", session.session_id)) {
                self.persist_code_session(session);
            }
        }
        self.session_storage_status()
    }

    pub fn delete_session_storage(&self, kind: &str, id: &str) -> Result<(), String> {
        if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
            return Err("Invalid conversation ID".into());
        }
        let _submission = self.chat_submission.lock();
        self.cancel_turn_signal(id);
        self.pending_messages.lock().remove(id);
        self.delete_subagents(id)?;
        let mut agents = self.agent_sessions.write();
        let mut codes = self.code_sessions.write();
        let _writer = self.storage.writer.lock();
        for path in [
            self.data_dir.join(kind).join(format!("{id}.json")),
            self.data_dir
                .join("recovery")
                .join(kind)
                .join(format!("{id}.json")),
        ] {
            if let Err(error) = fs::remove_file(path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    return Err(format!("Conversation could not be deleted: {error}"));
                }
            }
        }
        if kind == "sessions" {
            agents.remove(id);
        } else {
            codes.remove(id);
        }
        self.storage.issues.write().remove(&format!("{kind}/{id}"));
        drop(_writer);
        drop(codes);
        drop(agents);
        self.emit("quixi:storage-status", self.session_storage_status());
        Ok(())
    }

    pub(crate) fn load_session_recovery(&self) {
        for kind in ["sessions", "code_sessions"] {
            let Ok(entries) = fs::read_dir(self.data_dir.join("recovery").join(kind)) else {
                continue;
            };
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let Ok(raw) = fs::read(entry.path()) else {
                    continue;
                };
                let Ok(document) = serde_json::from_slice::<Value>(&raw) else {
                    continue;
                };
                let Some(id) = document
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .map(str::to_string)
                else {
                    continue;
                };
                if entry.file_name().to_str() != Some(&format!("{id}.json")) {
                    continue;
                }
                let primary = fs::read(self.data_dir.join(kind).join(format!("{id}.json")))
                    .ok()
                    .and_then(|raw| serde_json::from_slice::<Value>(&raw).ok());
                let revision =
                    |value: &Value| value["_quixiPersistenceRevision"].as_u64().unwrap_or(0);
                self.storage
                    .revision
                    .fetch_max(revision(&document), Ordering::SeqCst);
                if primary
                    .as_ref()
                    .is_some_and(|value| revision(value) >= revision(&document))
                {
                    continue;
                }
                if kind == "sessions" {
                    if let Ok(mut session) = serde_json::from_value::<LocalAgentSession>(document) {
                        session.is_running = false;
                        self.persist_agent_session(&session);
                        self.agent_sessions.write().insert(id.to_string(), session);
                    }
                } else if let Ok(mut session) =
                    serde_json::from_value::<LocalCodeSession>(document.clone())
                {
                    session.is_running = false;
                    session.turn_running = false;
                    self.code_transcripts.write().insert(
                        id.to_string(),
                        document["transcript"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default(),
                    );
                    self.persist_code_session(&session);
                    self.code_sessions.write().insert(id.to_string(), session);
                }
            }
        }
        // Retain the legacy file until the final (possibly recovered) transcript
        // has a durable canonical record. A failed migration must not destroy it.
        for session in self.code_sessions.read().values() {
            let Some(suffix) = session.session_id.strip_prefix("local_") else {
                continue;
            };
            let legacy = self
                .data_dir
                .join("code_sessions")
                .join(format!("local_cli_{suffix}.json"));
            if legacy.exists() {
                self.persist_code_session(session);
                if !self
                    .storage
                    .issues
                    .read()
                    .contains_key(&format!("code_sessions/{}", session.session_id))
                {
                    let _ = fs::remove_file(legacy);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PathBuf, Arc<QuixiEngine>, String) {
        let root = std::env::temp_dir().join(format!("quixi-storage-{}", Uuid::new_v4()));
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        let id = engine.start_agent_session(&json!({"title":"storage fixture"}));
        (root, engine, id)
    }

    fn obstruct(root: &Path, kind: &str) {
        fs::rename(root.join(kind), root.join(format!("original-{kind}"))).unwrap();
        fs::write(root.join(kind), b"not a directory").unwrap();
    }

    fn restore(root: &Path, kind: &str) {
        fs::remove_file(root.join(kind)).unwrap();
        fs::rename(root.join(format!("original-{kind}")), root.join(kind)).unwrap();
    }

    #[test]
    fn failed_save_recovers_latest_conversation_after_restart_and_retry() {
        let (root, engine, id) = fixture();
        let original = fs::read(root.join("sessions").join(format!("{id}.json"))).unwrap();
        obstruct(&root, "sessions");
        let mut session = engine.agent_sessions.read()[&id].clone();
        session.title = "latest unsaved title".into();
        session
            .buffered_messages
            .push(json!({"text":"latest Unicode café 🌲"}));
        engine.persist_agent_session(&session);
        assert_eq!(
            engine.session_storage_status()["issues"][0]["recoverySaved"],
            true
        );
        assert!(!engine.has_unsaved_conversations());
        assert_eq!(
            fs::read(root.join("original-sessions").join(format!("{id}.json"))).unwrap(),
            original
        );
        drop(engine);
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert_eq!(
            engine.agent_sessions.read()[&id].title,
            "latest unsaved title"
        );
        assert_eq!(
            engine.agent_sessions.read()[&id].buffered_messages,
            session.buffered_messages
        );
        restore(&root, "sessions");
        assert_eq!(engine.retry_session_saves()["issues"], json!([]));
        assert!(!root
            .join("recovery/sessions")
            .join(format!("{id}.json"))
            .exists());
        drop(engine);
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert_eq!(engine.agent_sessions.read()[&id].title, session.title);
        assert_eq!(
            engine.agent_sessions.read()[&id].buffered_messages,
            session.buffered_messages
        );
        drop(engine);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failure_of_primary_and_recovery_remains_unsaved_until_retry_succeeds() {
        let (root, engine, id) = fixture();
        engine.start_agent_session(&json!({"title":"unchanged clean session"}));
        obstruct(&root, "sessions");
        fs::write(root.join("recovery"), b"unavailable recovery").unwrap();
        {
            let mut sessions = engine.agent_sessions.write();
            let session = sessions.get_mut(&id).unwrap();
            session.title = "in-memory change".into();
            engine.persist_agent_session(session);
        }
        assert!(engine.has_unsaved_conversations());
        assert_eq!(
            engine.session_storage_status()["issues"][0]["recoverySaved"],
            false
        );
        assert_eq!(
            engine.session_storage_export()["sessions"]
                .as_array()
                .unwrap()
                .len(),
            1,
            "Recovery export must contain only affected conversations"
        );
        engine.retry_session_saves();
        assert_eq!(
            engine.session_storage_status()["issues"]
                .as_array()
                .unwrap()
                .len(),
            1,
            "Retry must not resave unaffected conversations"
        );
        restore(&root, "sessions");
        engine.retry_session_saves();
        assert!(!engine.has_unsaved_conversations());
        assert_eq!(engine.session_storage_status()["issues"], json!([]));
        drop(engine);
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert_eq!(engine.agent_sessions.read()[&id].title, "in-memory change");
        drop(engine);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn code_session_recovery_includes_transcript_and_stale_recovery_cannot_replace_primary() {
        let (root, engine, _) = fixture();
        let id = engine.start_code_session(&json!({"title":"code storage fixture"}));
        let session = engine.code_sessions.read()[&id].clone();
        engine
            .code_transcripts
            .write()
            .insert(id.clone(), vec![json!({"text":"code transcript café"})]);
        if !root.join("code_sessions").exists() {
            fs::create_dir_all(root.join("code_sessions")).unwrap();
        }
        obstruct(&root, "code_sessions");
        engine.persist_code_session(&session);
        let recovery_path = root
            .join("recovery/code_sessions")
            .join(format!("{id}.json"));
        let stale = fs::read(&recovery_path).unwrap();
        drop(engine);
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert_eq!(
            engine.code_transcripts.read()[&id][0]["text"],
            "code transcript café"
        );
        restore(&root, "code_sessions");
        engine
            .code_transcripts
            .write()
            .get_mut(&id)
            .unwrap()
            .push(json!({"text":"newer primary"}));
        engine.retry_session_saves();
        fs::write(recovery_path, stale).unwrap();
        drop(engine);
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert_eq!(engine.code_transcripts.read()[&id].len(), 2);
        assert_eq!(
            engine.code_transcripts.read()[&id][1]["text"],
            "newer primary"
        );
        drop(engine);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deleting_session_removes_recovery_so_restart_cannot_resurrect_it() {
        let (root, engine, id) = fixture();
        let recovery = root.join("recovery/sessions");
        fs::create_dir_all(&recovery).unwrap();
        fs::copy(
            root.join("sessions").join(format!("{id}.json")),
            recovery.join(format!("{id}.json")),
        )
        .unwrap();
        engine.delete_session_storage("sessions", &id).unwrap();
        assert!(!recovery.join(format!("{id}.json")).exists());
        drop(engine);
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert!(!engine.agent_sessions.read().contains_key(&id));
        drop(engine);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_code_session_is_only_retired_after_canonical_transcript_is_saved() {
        let (root, engine, _) = fixture();
        let id = engine.start_code_session(&json!({"title":"legacy conversation"}));
        let canonical = root.join("code_sessions").join(format!("{id}.json"));
        let legacy_id = id.replacen("local_", "local_cli_", 1);
        let legacy = root.join("code_sessions").join(format!("{legacy_id}.json"));
        let mut document: Value = serde_json::from_slice(&fs::read(&canonical).unwrap()).unwrap();
        document["sessionId"] = json!(legacy_id);
        document["transcript"] = json!([{"text":"preserve migrated transcript"}]);
        fs::write(&legacy, serde_json::to_vec(&document).unwrap()).unwrap();
        fs::remove_file(&canonical).unwrap();
        drop(engine);
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert!(!legacy.exists());
        assert!(canonical.exists());
        assert_eq!(
            engine.code_transcripts.read()[&id][0]["text"],
            "preserve migrated transcript"
        );
        drop(engine);
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert_eq!(
            engine.code_transcripts.read()[&id][0]["text"],
            "preserve migrated transcript"
        );
        drop(engine);
        fs::remove_dir_all(root).unwrap();
    }
}
