//! Local search and private HTML artifacts shared by IPC and the agent tool loop.
use super::*;

fn matching_snippet(query: &str, title: &str, messages: &[Value]) -> Option<String> {
    std::iter::once(title.to_string())
        .chain(messages.iter().flat_map(|m| {
            m.pointer("/message/content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|b| b.get("text").and_then(Value::as_str).map(str::to_owned))
        }))
        .find(|text| text.to_lowercase().contains(query))
        .map(|text| {
            // Center the snippet on the match, retaining Unicode boundaries.
            let chars: Vec<char> = text.chars().collect();
            let offset = text.to_lowercase().find(query).unwrap_or(0);
            let start = text.to_lowercase()[..offset]
                .chars()
                .count()
                .saturating_sub(60);
            chars.into_iter().skip(start).take(240).collect()
        })
}

impl QuixiEngine {
    /// Replace only account-facing bootstrap data; provider credentials remain separate.
    /// Called at startup for an explicitly selected test profile, never by renderer IPC.
    pub fn apply_mock_account(&self, fixture: &Value) -> Result<(), String> {
        let account = fixture.get("account").filter(|v| v.is_object()).ok_or("Mock fixture requires account object")?;
        let id = account.get("uuid").and_then(Value::as_str).ok_or("Mock account requires uuid")?;
        Uuid::parse_str(id).map_err(|_| "Mock account uuid must be a UUID")?;
        if !account.get("memberships").is_some_and(Value::is_array) {
            return Err("Mock account requires memberships array".into());
        }
        let mut boot = self.bootstrap_template.write();
        boot["account"] = account.clone();
        if let Some(settings) = account.get("settings").filter(|v| v.is_object()) {
            *self.account_settings.write() = settings.clone();
        }
        for key in ["current_user_access", "featureFlagsOrgUuid"] {
            if let Some(value) = fixture.get(key) { boot[key] = value.clone(); }
        }
        Ok(())
    }

    pub fn search_local_sessions(&self, query: &str, limit: usize) -> Vec<Value> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return vec![];
        }
        let mut matches: Vec<(u64, Value)> = self
            .agent_sessions
            .read()
            .values()
            .filter_map(|s| {
                matching_snippet(&query, &s.title, &s.buffered_messages).map(|snippet| {
                    (
                        s.last_activity_at,
                        json!({"sessionId":s.session_id,"snippet":snippet}),
                    )
                })
            })
            .collect();
        let transcripts = self.code_transcripts.read();
        matches.extend(self.code_sessions.read().values().filter_map(|s| {
            matching_snippet(
                &query,
                &s.title,
                transcripts
                    .get(&s.session_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
            )
            .map(|snippet| {
                (
                    s.last_activity_at,
                    json!({"sessionId":s.session_id,"snippet":snippet}),
                )
            })
        }));
        matches.sort_by(|a, b| b.0.cmp(&a.0));
        let mut results: Vec<_> = matches.into_iter().take(limit).map(|(_, v)| v).collect();
        if results.len() >= limit {
            return results;
        }
        for result in quixi_workspace::TranscriptSearchEngine::search_transcripts(&query, limit) {
            if results.len() >= limit {
                break;
            }
            if !results
                .iter()
                .any(|r| r["sessionId"] == result["sessionId"])
            {
                results.push(result);
            }
        }
        results
    }

    pub(crate) fn load_local_artifacts(&self) {
        if let Ok(entries) = fs::read_dir(self.data_dir.join("artifacts")) {
            for entry in entries
                .flatten()
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
            {
                if let Ok(raw) = fs::read(entry.path()) {
                    if let Ok(artifact) = serde_json::from_slice::<Value>(&raw) {
                        if let Some(id) = artifact["id"]
                            .as_str()
                            .filter(|id| Uuid::parse_str(id).is_ok())
                        {
                            self.artifacts.write().insert(id.to_string(), artifact);
                        }
                    }
                }
            }
        }
    }

    pub fn update_local_artifact(&self, id: &str, key: &str, value: Value) -> bool {
        let mut artifacts = self.artifacts.write();
        let Some(current) = artifacts.get(id) else {
            return false;
        };
        if Uuid::parse_str(id).is_err() {
            return false;
        }
        let mut updated = current.clone();
        updated[key] = value.clone();
        if key == "name" {
            updated["title"] = value;
        }
        updated["updatedAt"] = json!(Utc::now().timestamp_millis());
        if fs::write(
            self.data_dir.join("artifacts").join(format!("{id}.json")),
            serde_json::to_vec(&updated).unwrap(),
        )
        .is_err()
        {
            return false;
        }
        artifacts.insert(id.to_string(), updated);
        drop(artifacts);
        self.emit_eipc(
            "claude.web",
            "CoworkArtifacts",
            "onArtifactsChanged",
            json!({}),
        );
        true
    }

    pub fn delete_local_artifact(&self, id: &str) -> bool {
        if Uuid::parse_str(id).is_err() || !self.artifacts.read().contains_key(id) {
            return false;
        }
        if fs::remove_file(self.data_dir.join("artifacts").join(format!("{id}.json"))).is_err() {
            return false;
        }
        self.artifacts.write().remove(id);
        self.emit_eipc(
            "claude.web",
            "CoworkArtifacts",
            "onArtifactsChanged",
            json!({}),
        );
        true
    }

    pub fn create_local_artifact(&self, session_id: &str, input: &Value) -> (String, bool) {
        let title = input["title"].as_str().unwrap_or("").trim();
        let html = input["html"].as_str().unwrap_or("");
        if title.is_empty() || html.trim().is_empty() || html.len() > 2_000_000 {
            return (
                "Artifact requires a title and nonempty HTML (maximum 2 MB).".into(),
                true,
            );
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();
        let artifact = json!({"id":id,"name":title,"title":title,"description":"Private local HTML preview",
            "html":html,"createdAt":now,"updatedAt":now,"createdBySessionType":"cowork",
            "createdBySessionId":session_id,"isStarred":false});
        let path = self.data_dir.join("artifacts").join(format!("{id}.json"));
        if let Err(err) = fs::write(path, serde_json::to_vec(&artifact).unwrap()) {
            return (format!("Could not save local artifact: {err}"), true);
        }
        self.artifacts.write().insert(id.clone(), artifact);
        self.emit_eipc(
            "claude.web",
            "CoworkArtifacts",
            "onArtifactsChanged",
            json!({}),
        );
        self.emit("quixi:artifact-preview", json!({"id":id}));
        (format!("Created private local preview: [{title}](app://localhost/cowork-artifact/{id}). Nothing was published or uploaded."), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mock_accounts_are_profile_local_and_invalid_fixtures_do_not_replace_them() {
        let roots: Vec<_>=(0..2).map(|_| std::env::temp_dir().join(format!("quixi-account-{}",Uuid::new_v4()))).collect();
        let engines: Vec<_>=roots.iter().map(|p| QuixiEngine::new_with_data_dir(PathBuf::new(),p.clone())).collect();
        for (index,engine) in engines.iter().enumerate() {
            let fixture=json!({"account":{"uuid":Uuid::new_v4().to_string(),"email_address":format!("qa{index}@example.invalid"),"full_name":format!("Account {index}"),"memberships":[],"settings":{"marker":index}}});
            engine.apply_mock_account(&fixture).unwrap();
            assert_eq!(engine.build_live_bootstrap()["account"]["settings"]["marker"],index);
            assert_eq!(engine.mcp_host.config_path.parent(),Some(roots[index].as_path()));
            assert_eq!(engine.netstack.config_file.parent(),Some(roots[index].as_path()));
            assert_eq!(engine.provider_manager.get_public_state()["configuredCount"],0);
            assert!(engine.apply_mock_account(&json!({"account":{"uuid":"bad","memberships":[]}})).is_err());
            assert_eq!(engine.build_live_bootstrap()["account"]["email_address"],format!("qa{index}@example.invalid"));
        }
        drop(engines);for root in roots { fs::remove_dir_all(root).unwrap(); }
    }
    #[test]
    fn project_instructions_reach_provider_without_leaking_to_other_projects() {
        use std::io::{Read, Write};
        let root = std::env::temp_dir().join(format!("quixi-project-context-{}", Uuid::new_v4()));
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        engine.cowork_spaces.write().get_mut("space_quixidesk_default").unwrap().instructions = "MAPLE-PROJECT-CONTEXT".into();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut data = Vec::new(); let mut buf = [0;4096];
                loop {
                    let n = socket.read(&mut buf).unwrap(); if n==0 { break; } data.extend_from_slice(&buf[..n]);
                    if let Some(end) = data.windows(4).position(|w| w==b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&data[..end]);
                        let length:usize = headers.lines().find_map(|l| l.split_once(':').filter(|(k,_)| k.eq_ignore_ascii_case("content-length")).map(|(_,v)|v.trim().parse().unwrap())).unwrap();
                        if data.len()>=end+4+length { requests.push(serde_json::from_slice::<Value>(&data[end+4..end+4+length]).unwrap()); break; }
                    }
                }
                let event = json!({"choices":[{"delta":{"content":"OK"},"finish_reason":null}]});
                let body = format!("data: {event}\n\ndata: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\ndata: [DONE]\n\n");
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            }
            requests
        });
        engine.provider_manager.save_provider(&json!({"providerId":"custom","baseUrl":endpoint,"apiFormat":"openai","selectedModel":"context-fixture","setAsActive":true}));
        for project in [Some("space_quixidesk_default"),None] {
            let sid=engine.start_agent_session(&json!({"spaceId":project}));
            let (text,_)=engine.generate_reply_with_tools_streaming("token?","/tmp",&[],None,"default",&sid,&AtomicBool::new(false), |_|{}, |_|{});
            assert_eq!(text,"OK");
        }
        let requests=server.join().unwrap();
        assert!(requests[0]["messages"][0]["content"].as_str().unwrap().contains("MAPLE-PROJECT-CONTEXT"));
        assert!(!requests[1]["messages"][0]["content"].as_str().unwrap().contains("MAPLE-PROJECT-CONTEXT"));
        drop(engine); fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn searches_new_sessions_without_an_index_and_after_restart() {
        let root = std::env::temp_dir().join(format!("quixi-search-test-{}", Uuid::new_v4()));
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        let sid = engine.start_agent_session(&json!({"title":"QA grocery"}));
        {
            let mut sessions = engine.agent_sessions.write();
            let session = sessions.get_mut(&sid).unwrap();
            session.buffered_messages.push(json!({"type":"user","message":{"content":[{"type":"text","text":"Remember spinach and 水"}]}}));
            engine.persist_agent_session(session);
        }
        assert_eq!(
            engine.search_local_sessions("spinach", 1)[0]["sessionId"],
            sid
        );
        assert_eq!(
            engine.search_local_sessions("QA GROCERY", 1)[0]["sessionId"],
            sid
        );
        assert!(engine.search_local_sessions("  ", 1).is_empty());
        drop(engine);
        let reopened = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert_eq!(reopened.search_local_sessions("水", 1)[0]["sessionId"], sid);
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_artifacts_survive_restart_and_invalid_input_is_rejected() {
        let root = std::env::temp_dir().join(format!("quixi-artifact-test-{}", Uuid::new_v4()));
        let engine = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert!(
            engine
                .create_local_artifact("qa", &json!({"title":"Empty"}))
                .1
        );
        assert!(engine.artifacts.read().is_empty());
        let (result, failed) = engine.create_local_artifact(
            "qa",
            &json!({"title":"Picnic", "html":"<h1>Picnic</h1><p>水</p>"}),
        );
        assert!(!failed, "{result}");
        let id = engine.artifacts.read().keys().next().unwrap().clone();
        assert!(engine.update_local_artifact(&id, "name", json!("Picnic renamed")));
        drop(engine);
        let reopened = QuixiEngine::new_with_data_dir(PathBuf::new(), root.clone());
        assert_eq!(reopened.artifacts.read()[&id]["name"], "Picnic renamed");
        assert!(reopened.artifacts.read()[&id]["html"]
            .as_str()
            .unwrap()
            .contains('水'));
        assert!(result.contains("Nothing was published"));
        assert!(reopened.delete_local_artifact(&id));
        assert!(!root.join("artifacts").join(format!("{id}.json")).exists());
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn content_search_reads_real_messages_and_handles_unicode() {
        let messages = vec![
            json!({"message":{"content":[{"type":"text","text":"A picnic grocery list: 水, sandwiches, blanket"}]}}),
        ];
        assert!(matching_snippet("grocery", "New chat", &messages)
            .unwrap()
            .contains("grocery"));
        assert!(matching_snippet("水", "New chat", &messages)
            .unwrap()
            .contains('水'));
        assert_eq!(matching_snippet("missing", "New chat", &messages), None);
        assert_eq!(
            matching_snippet("grocery", "Grocery", &[]),
            Some("Grocery".into())
        );
    }
}
