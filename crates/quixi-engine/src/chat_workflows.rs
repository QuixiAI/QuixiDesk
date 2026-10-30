//! Validated attachments and non-destructive conversation branches.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;

const MAX_FILES: usize = 8;
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 20 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Attachment {
    name: String,
    mime_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    data: Option<String>,
}

pub(crate) fn needs_workspace(name: &str) -> bool {
    matches!(name, "Read" | "Write" | "Edit" | "Grep" | "ListDirectory" | "Bash")
}

fn image_signature(bytes: &[u8], mime: &str) -> bool {
    match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        _ => false,
    }
}

fn prepare_content(text: &str, attachments: Option<&Value>) -> Result<Vec<Value>, String> {
    if text.len() > MAX_TEXT_BYTES { return Err("Message is too large (maximum 1 MB).".into()); }
    let attachments: Vec<Attachment> = match attachments {
        Some(v) => serde_json::from_value(v.clone()).map_err(|_| "Invalid attachment metadata. Reattach the file.".to_string())?,
        None => Vec::new(),
    };
    if attachments.len() > MAX_FILES { return Err("Attach up to 8 files per message.".into()); }
    if text.trim().is_empty() && attachments.is_empty() { return Err("Write a message or attach a file.".into()); }
    let mut blocks = vec![json!({"type":"text","text":text})];
    let mut total = 0;
    for file in attachments {
        if file.name.trim().is_empty() || file.name.len() > 255 || file.name.chars().any(char::is_control) {
            return Err("Attachment has an invalid file name.".into());
        }
        let mime = file.mime_type.as_str();
        if matches!(mime, "image/png" | "image/jpeg" | "image/gif" | "image/webp") {
            if file.text.is_some() { return Err(format!("{}: image attachments must contain image data.", file.name)); }
            let data = file.data.ok_or_else(||format!("{}: image data is missing.", file.name))?;
            if data.len() > MAX_IMAGE_BYTES.div_ceil(3)*4 { return Err(format!("{} exceeds the 5 MB image limit.", file.name)); }
            let bytes = STANDARD.decode(&data).map_err(|_|format!("{}: invalid image encoding.",file.name))?;
            if bytes.len()>MAX_IMAGE_BYTES || !image_signature(&bytes,mime) { return Err(format!("{}: invalid {} image or exceeds 5 MB.",file.name,mime)); }
            total += bytes.len();
            blocks.push(json!({"type":"image_url","image_url":{"url":format!("data:{mime};base64,{data}")},"attachment":{"name":file.name,"mimeType":mime,"size":bytes.len()}}));
        } else if mime.starts_with("text/") || matches!(mime,"application/json"|"application/xml"|"application/javascript"|"application/x-yaml") {
            if file.data.is_some() { return Err(format!("{}: text attachments must contain decoded UTF-8 text.",file.name)); }
            let body = file.text.ok_or_else(||format!("{}: file contents are missing.",file.name))?;
            if body.len()>MAX_TEXT_BYTES || body.contains('\0') { return Err(format!("{} must be a UTF-8 text file of at most 1 MB.",file.name)); }
            total += body.len();
            blocks.push(json!({"type":"text","text":format!("Attached file: {}\n\n{}",file.name,body),"attachment":{"name":file.name,"mimeType":mime,"size":body.len(),"text":body}}));
        } else {
            return Err(format!("{}: unsupported file type. Attach PNG, JPEG, GIF, WebP, or a UTF-8 text file.",file.name));
        }
        if total>MAX_TOTAL_BYTES { return Err("Attachments exceed the 20 MB total limit.".into()); }
    }
    Ok(blocks)
}

fn fork_point(messages: &[Value], uuid: &str) -> Result<usize,String> {
    let index=messages.iter().position(|message|message["uuid"].as_str()==Some(uuid)).ok_or("This message is no longer available.")?;
    let is_request=|message:&Value| message["type"]=="user" && message.pointer("/message/content").and_then(Value::as_array).is_some_and(|blocks|blocks.iter().any(|block|matches!(block["type"].as_str(),Some("text"|"image_url"))));
    if is_request(&messages[index]) { return Ok(index); }
    if messages[index]["type"]!="assistant" {return Err("Choose a user message or assistant reply to branch from.".into());}
    (0..index).rev().find(|&i|is_request(&messages[i])).ok_or("The original request for this reply is unavailable.".into())
}

impl QuixiEngine {
    /// An MCP process needs a working directory even for conversations without a selected folder.
    /// This private empty directory is not a workspace selection or filesystem tool grant.
    pub fn mcp_workspace(&self, cwd:&str)->PathBuf {
        if cwd.trim().is_empty() {self.data_dir.join("scratchpad/no-folder")} else {PathBuf::from(cwd)}
    }
    /// Serialize acceptance so concurrent clicks cannot append duplicate requests or cancel each other.
    pub fn send_chat_message(self: &Arc<Self>, session_id: &str, options: &Value) -> Result<Value,String> {
        let text=options["text"].as_str().unwrap_or_default();
        let content=prepare_content(text,options.get("attachments"))?;
        let _submission=self.chat_submission.lock();
        let agent=self.agent_sessions.read().get(session_id).cloned();
        let code=self.code_sessions.read().get(session_id).cloned();
        if agent.is_none() && code.is_none() {return Err("This conversation no longer exists. Start a new conversation.".into());}
        let uuid=options["messageUuid"].as_str().filter(|id|!id.trim().is_empty()).map(str::to_owned).unwrap_or_else(||Uuid::new_v4().to_string());
        let prior=agent.as_ref().map(|session|session.buffered_messages.clone()).unwrap_or_else(||self.code_transcripts.read().get(session_id).cloned().unwrap_or_default());
        if prior.iter().any(|message|message["uuid"].as_str()==Some(&uuid)) {return Ok(json!({"dispatched":true,"duplicate":true,"messageUuid":uuid}));}
        if agent.as_ref().is_some_and(|s|s.is_running) || code.as_ref().is_some_and(|s|s.turn_running) {return Err("This conversation is still running. Stop it or wait before sending another message.".into());}
        if agent.is_some() { self.send_agent_message_content(session_id,text,Some(&uuid),content); }
        else {self.send_code_message_content(session_id,text,Some(&uuid),content);}
        Ok(json!({"dispatched":true,"messageUuid":uuid}))
    }

    /// Edits and retries are new branches. The original transcript is never truncated.
    pub fn fork_chat_message(self: &Arc<Self>, session_id:&str, options:&Value) -> Result<Value,String> {
        let _submission=self.chat_submission.lock();
        let agent=self.agent_sessions.read().get(session_id).cloned();
        let code=self.code_sessions.read().get(session_id).cloned();
        if agent.is_none() && code.is_none() {return Err("This conversation no longer exists.".into());}
        if agent.as_ref().is_some_and(|s|s.is_running) || code.as_ref().is_some_and(|s|s.turn_running) {return Err("Stop the current response before editing or retrying.".into());}
        let messages=agent.as_ref().map(|s|s.buffered_messages.clone()).unwrap_or_else(||self.code_transcripts.read().get(session_id).cloned().unwrap_or_default());
        let target=options["messageUuid"].as_str().ok_or("Choose a message to edit or retry.")?;
        let index=fork_point(&messages,target)?;
        let mut content=messages[index]["message"]["content"].as_array().cloned().ok_or("This message cannot be retried.")?;
        let original=content.iter().find(|b|b["type"]=="text" && b.get("attachment").is_none()).and_then(|b|b["text"].as_str()).unwrap_or_default();
        let text=options.get("text").and_then(Value::as_str).unwrap_or(original).to_owned();
        if text.len()>MAX_TEXT_BYTES {return Err("Message is too large (maximum 1 MB).".into());}
        if let Some(attachments)=options.get("attachments") { content=prepare_content(&text,Some(attachments))?; }
        else {
            if text.trim().is_empty() && !content.iter().any(|b|b.get("attachment").is_some()) {return Err("Write a message or attach a file.".into());}
            if let Some(first)=content.iter_mut().find(|b|b["type"]=="text"&&b.get("attachment").is_none()) {first["text"]=json!(text);}
            else {content.insert(0,json!({"type":"text","text":text}));}
        }
        let sid=format!("local_{}",Uuid::new_v4());
        let origin=Some(quixi_proto::SessionForkOrigin{session_id:session_id.into(),message_uuid:messages[index]["uuid"].as_str().unwrap_or(target).into()});
        let mut history=messages[..index].to_vec();
        for message in &mut history {message["session_id"]=json!(sid);}
        let now=Utc::now().timestamp_millis() as u64;
        if let Some(mut session)=agent {
            session.session_id=sid.clone();session.forked_from=origin;session.cli_session_id=Some(Uuid::new_v4().to_string());
            session.title=format!("{} · branch",session.title.trim_end_matches(" · branch"));
            session.created_at=now;session.last_activity_at=now;session.is_archived=false;session.is_starred=false;
            session.error=None;session.pending_tool_permissions.clear();session.buffered_messages=history;
            self.persist_agent_session(&session);self.agent_sessions.write().insert(sid.clone(),session);
            self.send_agent_message_content(&sid,&text,None,content);
            Ok(json!(self.agent_sessions.read().get(&sid)))
        } else {
            let mut session=code.unwrap();session.session_id=sid.clone();session.forked_from=origin;
            session.title=format!("{} · branch",session.title.trim_end_matches(" · branch"));session.created_at=now;session.last_activity_at=now;
            session.is_archived=false;session.is_starred=false;
            self.code_transcripts.write().insert(sid.clone(),history);self.persist_code_session(&session);self.code_sessions.write().insert(sid.clone(),session);
            self.send_code_message_content(&sid,&text,None,content);
            Ok(json!(self.code_sessions.read().get(&sid)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_request_reaches_openai_wire_once_and_folderless_tools_stay_unavailable() {
        use std::io::{BufRead,Read,Write};
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url=format!("http://{}/v1",listener.local_addr().unwrap());
        let (sent,received)=std::sync::mpsc::channel();
        let server=std::thread::spawn(move||{
            for _ in 0..2 {
            let (mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut reader=std::io::BufReader::new(stream.try_clone().unwrap());let mut length=0;
            loop {let mut line=String::new();reader.read_line(&mut line).unwrap();if line=="\r\n"{break;}if let Some(value)=line.to_lowercase().strip_prefix("content-length:"){length=value.trim().parse().unwrap();}}
            let mut bytes=vec![0;length];reader.read_exact(&mut bytes).unwrap();sent.send(serde_json::from_slice::<Value>(&bytes).unwrap()).unwrap();
            let body="data: {\"choices\":[{\"delta\":{\"content\":\"I received the image\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            }
        });
        let root=std::env::temp_dir().join(format!("quixi-image-wire-{}",Uuid::new_v4()));
        let engine=QuixiEngine::new_with_data_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"),root.clone());
        assert!(engine.provider_manager.save_provider(&json!({"providerId":"custom","baseUrl":url,"apiFormat":"openai","selectedModel":"fixture","activate":true})).get("error").is_none());
        let sid=engine.start_agent_session(&json!({"model":"fixture"}));
        let data=STANDARD.encode(b"\x89PNG\r\n\x1a\nfixture");
        engine.send_chat_message(&sid,&json!({"text":"Describe this","attachments":[{"name":"photo.png","mimeType":"image/png","data":data},{"name":"notes.txt","mimeType":"text/plain","text":"Image notes"}]})).unwrap();
        let request=received.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        let users=request["messages"].as_array().unwrap().iter().filter(|m|m["role"]=="user").collect::<Vec<_>>();
        assert_eq!(users.len(),1);assert_eq!(users[0]["content"][0]["text"],"Describe this");
        assert_eq!(users[0]["content"][1]["image_url"]["url"],format!("data:image/png;base64,{data}"));
        assert!(users[0]["content"][2]["text"].as_str().unwrap().contains("Image notes"));
        assert!(request["tools"].as_array().unwrap().iter().all(|tool|!needs_workspace(tool["function"]["name"].as_str().unwrap())));
        for _ in 0..100 {if !engine.agent_sessions.read()[&sid].is_running {break;}std::thread::sleep(std::time::Duration::from_millis(10));}
        assert!(!engine.agent_sessions.read()[&sid].is_running);
        let text_sid=engine.start_agent_session(&json!({"model":"fixture"}));
        engine.send_chat_message(&text_sid,&json!({"text":"Review notes","attachments":[{"name":"notes.txt","mimeType":"text/plain","text":"Only text"}]})).unwrap();
        let request=received.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        let users=request["messages"].as_array().unwrap().iter().filter(|m|m["role"]=="user").collect::<Vec<_>>();
        assert_eq!(users.len(),1);assert_eq!(users[0]["content"],"Review notes\nAttached file: notes.txt\n\nOnly text");
        server.join().unwrap();
        for _ in 0..100 {if !engine.agent_sessions.read()[&text_sid].is_running {break;}std::thread::sleep(std::time::Duration::from_millis(10));}
        assert!(!engine.agent_sessions.read()[&text_sid].is_running);drop(engine);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_files_are_rejected_and_images_reach_provider_history() {
        assert!(prepare_content("Hi",Some(&json!([{"name":"fake.png","mimeType":"image/png","data":STANDARD.encode(b"not an image")}] ))).is_err());
        assert!(prepare_content("Hi",Some(&json!([{"name":"book.pdf","mimeType":"application/pdf","text":"fake"}]))).is_err());
        assert!(prepare_content("Hi",Some(&json!([{"name":"big.txt","mimeType":"text/plain","text":"x".repeat(MAX_TEXT_BYTES+1)}]))).is_err());
        let data=STANDARD.encode(b"\x89PNG\r\n\x1a\nfixture");
        let content=prepare_content("Describe this",Some(&json!([{"name":"photo.png","mimeType":"image/png","data":data}]))).unwrap();
        let history=turns::provider_history(&[json!({"type":"user","message":{"content":content}})]);
        assert_eq!(history[0]["content"][0]["text"],"Describe this");
        assert_eq!(history[0]["content"][1]["image_url"]["url"],format!("data:image/png;base64,{data}"));
        assert!(history[0]["content"][1].get("attachment").is_none());
        assert!(context::estimated_tokens(&history)<4000);
    }
    #[test]
    fn forks_keep_original_and_reject_active_edits_and_duplicate_sends() {
        let root=std::env::temp_dir().join(format!("quixi-chat-{}",Uuid::new_v4()));
        let resources=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let engine=QuixiEngine::new_with_data_dir(resources,root.clone());
        let sid=engine.start_agent_session(&json!({}));
        let original=vec![json!({"type":"user","uuid":"u1","message":{"content":[{"type":"text","text":"first"}]}}),json!({"type":"assistant","uuid":"a1","message":{"content":[{"type":"text","text":"answer"}]}}),json!({"type":"user","uuid":"u2","message":{"content":[{"type":"text","text":"later"}]}})];
        engine.agent_sessions.write().get_mut(&sid).unwrap().buffered_messages=original.clone();
        assert_eq!(engine.agent_sessions.read()[&sid].cwd,"");
        assert!(engine.agent_sessions.read()[&sid].user_selected_folders.is_empty());
        let (error,failed)=engine.execute_tool_with_approval(&sid,"no-folder","Read",&json!({"file_path":"Cargo.toml"}),"","auto",&AtomicBool::new(false));
        assert!(failed && error.contains("No folder"));
        let branch=engine.fork_chat_message(&sid,&json!({"messageUuid":"a1","text":"edited first"})).unwrap();
        let new_id=branch["sessionId"].as_str().unwrap();engine.cancel_turn(new_id);
        assert_eq!(engine.agent_sessions.read()[&sid].buffered_messages,original);
        assert_eq!(branch["forkedFrom"]["messageUuid"],"u1");
        assert_eq!(branch["bufferedMessages"].as_array().unwrap().len(),1);
        assert_eq!(branch["bufferedMessages"][0]["message"]["content"][0]["text"],"edited first");
        engine.agent_sessions.write().get_mut(&sid).unwrap().is_running=true;
        assert!(engine.fork_chat_message(&sid,&json!({"messageUuid":"u1"})).is_err());
        assert!(engine.send_chat_message(&sid,&json!({"text":"next","messageUuid":"new"})).is_err());
        assert_eq!(engine.send_chat_message(&sid,&json!({"text":"first","messageUuid":"u1"})).unwrap()["duplicate"],true);
        assert_eq!(engine.agent_sessions.read()[&sid].buffered_messages,original);
        std::thread::sleep(std::time::Duration::from_millis(150));drop(engine);fs::remove_dir_all(root).unwrap();
    }
}
