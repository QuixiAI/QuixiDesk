//! Preserve visible response text when a user stops a turn.
use super::*;

/// Rebuild provider history with the evidence behind earlier answers. Every tool
/// call must have a result, including a call interrupted by Stop or a crash.
pub(crate) fn provider_history(transcript: &[Value]) -> Vec<Value> {
    let results: HashMap<&str, &Value> = transcript.iter()
        .filter_map(|m| m.pointer("/message/content").and_then(Value::as_array))
        .flatten().filter(|b| b["type"] == "tool_result")
        .filter_map(|b| b["tool_use_id"].as_str().map(|id| (id, b))).collect();
    let mut messages = Vec::new();
    for message in transcript {
        let Some(blocks) = message.pointer("/message/content").and_then(Value::as_array) else { continue; };
        let text = blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n");
        if message["type"] == "assistant" {
            let calls: Vec<_> = blocks.iter().filter(|b| b["type"] == "tool_use")
                .filter_map(|b| Some(json!({"id":b["id"].as_str()?,"type":"function",
                    "function":{"name":b["name"].as_str()?,"arguments":serde_json::to_string(&b["input"]).ok()?}}))).collect();
            if calls.is_empty() {
                if !text.trim().is_empty() { messages.push(json!({"role":"assistant","content":text})); }
            } else {
                messages.push(json!({"role":"assistant","content":if text.is_empty(){Value::Null}else{json!(text)},"tool_calls":calls}));
                for call in calls {
                    let id = call["id"].as_str().unwrap();
                    let output = match results.get(id) {
                        Some(result) => if let Some(text) = result["content"].as_str() { text.to_owned() } else { result["content"].to_string() },
                        None => "Action interrupted. No result was recorded; do not assume it succeeded.".into(),
                    };
                    messages.push(json!({"role":"tool","tool_call_id":id,"content":output}));
                }
            }
        } else if message["type"] == "user" && !text.trim().is_empty() {
            messages.push(json!({"role":"user","content":text}));
        }
    }
    messages
}

impl QuixiEngine {
    pub(crate) fn record_partial_reply(&self,sid:&str,signal:&AtomicBool,text:&str) {
        let mut partial=self.partial_replies.write();
        if let Some((current,body))=partial.get_mut(sid) {
            if std::ptr::eq(current.as_ref(),signal) && !signal.load(Ordering::SeqCst) {body.push_str(text);}
        }
    }
    pub(crate) fn clear_partial_reply(&self,sid:&str,signal:&AtomicBool) {
        if let Some((current,body))=self.partial_replies.write().get_mut(sid) {
            if std::ptr::eq(current.as_ref(),signal) {body.clear();}
        }
    }
    pub(crate) fn save_stopped_partial(&self,sid:&str) {
        let Some((_,text))=self.partial_replies.write().remove(sid) else {return;};
        if text.trim().is_empty() {return;}
        let message=json!({"type":"assistant","uuid":Uuid::new_v4().to_string(),"session_id":sid,
            "timestamp":Utc::now().to_rfc3339(),"message":{"role":"assistant","content":[{"type":"text","text":text}],"stop_reason":"user_stop"}});
        let already_saved=|messages:&[Value]| messages.last().is_some_and(|m|m["type"]=="assistant" && m.pointer("/message/content/0/text").and_then(Value::as_str)==Some(&text));
        if let Some(session)=self.agent_sessions.write().get_mut(sid) {
            if !already_saved(&session.buffered_messages) {session.buffered_messages.push(message);self.persist_agent_session(session);}
        } else if self.code_sessions.read().contains_key(sid) {
            {
                let mut transcripts=self.code_transcripts.write();
                let messages=transcripts.entry(sid.into()).or_default();
                if !already_saved(messages) {messages.push(message);}
            }
            if let Some(session)=self.code_sessions.read().get(sid) {self.persist_code_session(session);}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn followups_keep_tool_inputs_results_and_interrupted_calls() {
        let messages = provider_history(&[
            json!({"type":"user","message":{"content":[{"type":"text","text":"Edit the README"}]}}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"Updating it."},{"type":"tool_use","id":"edit-1","name":"Edit","input":{"file_path":"README.md"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"edit-1","content":"Successfully edited README.md","is_error":false}]}}),
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"stopped","name":"Bash","input":{"command":"test"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"text","text":"What changed?"}]}}),
        ]);
        assert_eq!(messages.len(), 6);
        assert_eq!(messages[1]["tool_calls"][0]["function"]["name"], "Edit");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["content"], "Successfully edited README.md");
        assert!(messages[4]["content"].as_str().unwrap().contains("interrupted"));
        assert_eq!(messages[5]["content"], "What changed?");
    }
    #[test]
    fn stopping_keeps_partial_text_after_restart_and_rejects_old_chunks() {
        let root=std::env::temp_dir().join(format!("quixi-stop-partial-{}",Uuid::new_v4()));
        let resources=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let engine=QuixiEngine::new_with_data_dir(resources.clone(),root.clone());
        let sid=engine.start_agent_session(&json!({}));
        let old=engine.begin_turn(&sid);engine.record_partial_reply(&sid,&old,"Keep this partial response.");engine.cancel_turn(&sid);
        assert_eq!(engine.agent_sessions.read()[&sid].buffered_messages.last().unwrap()["message"]["stop_reason"],"user_stop");
        let next=engine.begin_turn(&sid);engine.record_partial_reply(&sid,&old,"STALE");engine.record_partial_reply(&sid,&next,"Second response.");engine.cancel_turn(&sid);
        let count=engine.agent_sessions.read()[&sid].buffered_messages.len();assert_eq!(count,2);
        drop(engine);
        let engine=QuixiEngine::new_with_data_dir(resources,root.clone());
        let session=engine.agent_sessions.read();let messages=&session[&sid].buffered_messages;
        assert_eq!(messages.len(),2);assert_eq!(messages[0]["message"]["content"][0]["text"],"Keep this partial response.");assert_eq!(messages[1]["message"]["content"][0]["text"],"Second response.");
        drop(session);drop(engine);fs::remove_dir_all(root).unwrap();
    }
}
