//! In-turn approvals: keep Manual useful without silently granting actions.
use super::*;
use std::sync::atomic::AtomicU8;
use std::time::{Duration, Instant};

pub(crate) struct PendingToolApproval {
    session_id: String,
    request: Value,
    decision: Arc<AtomicU8>,
}

// Resolve existing symlinks and normalize missing descendants before checking scope.
fn file_access_outside_workspace(name:&str,input:&Value,cwd:&str)->bool {
    if !matches!(name,"Read"|"ListDirectory"|"Grep"|"Write"|"Edit") {return false;}
    let raw=input[if matches!(name,"ListDirectory"|"Grep") {"path"}else{"file_path"}].as_str().unwrap_or(".");
    let path=Path::new(raw);let path=if path.is_absolute(){path.to_path_buf()}else{Path::new(cwd).join(path)};
    fn resolved(path:&Path)->Option<PathBuf>{
        if let Ok(p)=path.canonicalize(){return Some(p);}
        let name=path.file_name()?;let parent=resolved(path.parent()?)?;Some(parent.join(name))
    }
    match (Path::new(cwd).canonicalize(),resolved(&path)) {(Ok(root),Some(path))=>!path.starts_with(root),_=>true}
}

impl QuixiEngine {
    fn permission_event(&self, session_id: &str, kind: &str) {
        let service = if self.agent_sessions.read().contains_key(session_id) {
            "LocalAgentModeSessions"
        } else { "LocalSessions" };
        self.emit_eipc("quixi.web", service, "onEvent", json!({"type":kind,"sessionId":session_id}));
    }

    pub fn pending_tool_approvals(&self, session_id: &str) -> Value {
        json!(self.pending_approvals.read().values()
            .filter(|p|p.session_id==session_id && matches!(p.decision.load(Ordering::SeqCst),0|3)).map(|p|p.request.clone()).collect::<Vec<_>>())
    }

    pub fn resolve_tool_approval(&self, session_id: &str, request_id: &str, decision: &str) -> Value {
        let (signal, file_edit) = {
            let pending=self.pending_approvals.read();
            let Some(p)=pending.get(request_id).filter(|p|p.session_id==session_id) else {
                return json!({"error":"This approval is no longer pending."});
            };
            (p.decision.clone(), matches!(p.request["toolName"].as_str(),Some("Write"|"Edit")) && p.request["outsideWorkspace"]!=true)
        };
        let answer=match decision {
            "allow_once" => 1,
            "allow_file_edits" if file_edit => 1,
            "deny" => 2,
            _ => return json!({"error":"Invalid approval choice."}),
        };
        // Reserve the request before changing mode, so it cannot be resolved twice.
        if signal.compare_exchange(0,3,Ordering::SeqCst,Ordering::SeqCst).is_err() {
            return json!({"error":"This approval was already answered."});
        }
        if decision=="allow_file_edits" {
            if let Some(s)=self.agent_sessions.write().get_mut(session_id) {
                s.permission_mode="acceptEdits".into(); self.persist_agent_session(s);
            } else if let Some(s)=self.code_sessions.write().get_mut(session_id) {
                s.permission_mode="acceptEdits".into(); self.persist_code_session(s);
            }
        }
        signal.store(answer,Ordering::SeqCst);
        self.permission_event(session_id,"tool_permission_changed");
        json!({"ok":true})
    }

    pub(crate) fn cancel_tool_approvals(&self, session_id: &str) {
        self.pending_approvals.write().retain(|_,p| {
            if p.session_id==session_id {p.decision.store(2,Ordering::SeqCst);false} else {true}
        });
    }

    pub(crate) fn execute_tool_with_approval(&self, session_id: &str, call_id: &str, name: &str,
        input: &Value, cwd: &str, fallback_mode: &str, cancelled: &AtomicBool) -> (String,bool) {
        if let Err(error)=agent_tools::validate_input(name,input) {return (error,true);}
        if cancelled.load(Ordering::SeqCst) {return ("Task stopped. No action performed.".into(),true);}
        let mcp_identity=if name==agent_tools::TOOL_MCP_CALL {
            match self.mcp_host.server_identity(input["server"].as_str().unwrap_or_default()) {
                Ok(identity)=>Some(identity),Err(error)=>return(format!("MCP server is unavailable: {error}"),true),
            }
        }else{None};
        let outside=file_access_outside_workspace(name,input,cwd);
        let needs=|mode:&str| agent_tools::requires_approval_with_host(name,input,mode,&self.mcp_host) || (outside&&!matches!(mode,"auto"|"bypassPermissions"));
        let mode=self.current_permission_mode(session_id,fallback_mode);
        if !needs(&mode) {
            return agent_tools::execute_cancellable_with_mcp_identity(name,input,cwd,&mode,&self.mcp_host,cancelled,mcp_identity.as_deref());
        }
        if cancelled.load(Ordering::SeqCst) {return ("Task stopped. No action performed.".into(),true);}
        let request_id=Uuid::new_v4().to_string();
        let signal=Arc::new(AtomicU8::new(0));
        self.pending_approvals.write().insert(request_id.clone(),PendingToolApproval {
            session_id:session_id.into(), decision:signal.clone(),
            request:json!({"requestId":request_id,"sessionId":session_id,"toolCallId":call_id,"toolName":name,"input":input,"cwd":cwd,"outsideWorkspace":outside,"mcpServerIdentity":mcp_identity,"subagent":self.subagent_for_call(session_id,call_id)}),
        });
        self.permission_event(session_id,"tool_permission_requested");
        let start=Instant::now();
        let outcome=loop {
            if cancelled.load(Ordering::SeqCst) {break 2;}
            let decision=signal.load(Ordering::SeqCst);
            if decision==1 || decision==2 {break decision;}
            if decision==0 && !needs(&self.current_permission_mode(session_id,fallback_mode)) {break 1;}
            if start.elapsed()>Duration::from_secs(30*60) {break 2;}
            std::thread::sleep(Duration::from_millis(50));
        };
        self.pending_approvals.write().remove(&request_id);
        self.permission_event(session_id,"tool_permission_changed");
        if outcome!=1 || cancelled.load(Ordering::SeqCst) {
            return ("The action was not approved or the task was stopped. No action was performed. Do not retry it without a new user request.".into(),true);
        }
        agent_tools::execute_cancellable_with_mcp_identity(name,input,cwd,"auto",&self.mcp_host,cancelled,mcp_identity.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Arc<QuixiEngine>,String,PathBuf) {
        let root=std::env::temp_dir().join(format!("quixi-approval-{}",Uuid::new_v4()));
        let engine=QuixiEngine::new_with_data_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"),root.join("data"));
        fs::create_dir_all(root.join("work")).unwrap();
        let sid=engine.start_agent_session(&json!({"cwd":root.join("work"),"userSelectedFolders":[root.join("work")],"permissionMode":"default"}));
        (engine,sid,root)
    }
    fn pending(engine:&QuixiEngine,sid:&str)->Value {
        let until=Instant::now()+Duration::from_secs(3);
        loop {let p=engine.pending_tool_approvals(sid);if !p.as_array().unwrap().is_empty(){return p[0].clone();}assert!(Instant::now()<until,"approval did not appear");std::thread::sleep(Duration::from_millis(10));}
    }
    #[test]
    fn manual_edit_waits_for_exact_approval_and_resumes_once() {
        let (engine,sid,root)=fixture();let cancelled=engine.begin_turn(&sid);let e=engine.clone();let id=sid.clone();let cwd=root.join("work");let path=cwd.join("README.md");
        let worker=std::thread::spawn(move|| e.execute_tool_with_approval(&id,"call-one","Write",&json!({"file_path":"README.md","content":"approved"}),cwd.to_str().unwrap(),"default",&cancelled));
        let p=pending(&engine,&sid);assert!(!path.exists());assert_eq!(p["input"]["content"],"approved");
        let request=p["requestId"].as_str().unwrap();assert!(engine.resolve_tool_approval("wrong-session",request,"allow_once")["error"].is_string());assert!(!path.exists());
        assert_eq!(engine.resolve_tool_approval(&sid,request,"allow_once")["ok"],true);
        assert!(!worker.join().unwrap().1);assert_eq!(fs::read_to_string(&path).unwrap(),"approved");
        assert_eq!(engine.agent_sessions.read()[&sid].permission_mode,"default");
        assert!(engine.resolve_tool_approval(&sid,request,"allow_once")["error"].is_string());
        drop(engine);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn allow_file_edits_resumes_task_but_keeps_shell_approval() {
        let (engine,sid,root)=fixture();let cancelled=engine.begin_turn(&sid);let e=engine.clone();let id=sid.clone();let cwd=root.join("work");let next_cwd=cwd.clone();
        let worker=std::thread::spawn(move||e.execute_tool_with_approval(&id,"edit-one","Write",&json!({"file_path":"README.md","content":"one"}),cwd.to_str().unwrap(),"default",&cancelled));
        let p=pending(&engine,&sid);assert_eq!(engine.resolve_tool_approval(&sid,p["requestId"].as_str().unwrap(),"allow_file_edits")["ok"],true);assert!(!worker.join().unwrap().1);
        assert_eq!(engine.current_permission_mode(&sid,"default"),"acceptEdits");
        assert!(!engine.execute_tool_with_approval(&sid,"edit-two","Write",&json!({"file_path":"example.txt","content":"two"}),next_cwd.to_str().unwrap(),"default",&AtomicBool::new(false)).1);
        assert!(agent_tools::requires_approval("Bash",&json!({"command":"echo test"}),"acceptEdits"));
        assert!(engine.pending_tool_approvals(&sid).as_array().unwrap().is_empty());drop(engine);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn denial_and_stop_never_perform_the_write() {
        let (engine,sid,root)=fixture();
        for deny in [true,false] {
            let cancelled=engine.begin_turn(&sid);let e=engine.clone();let id=sid.clone();let cwd=root.join("work");
            let worker=std::thread::spawn(move||e.execute_tool_with_approval(&id,"blocked","Write",&json!({"file_path":"never.txt","content":"no"}),cwd.to_str().unwrap(),"default",&cancelled));
            let p=pending(&engine,&sid);
            if deny {assert_eq!(engine.resolve_tool_approval(&sid,p["requestId"].as_str().unwrap(),"deny")["ok"],true);} else {engine.cancel_turn(&sid);}
            assert!(worker.join().unwrap().1);assert!(!root.join("work/never.txt").exists());assert!(engine.pending_tool_approvals(&sid).as_array().unwrap().is_empty());
        }
        drop(engine);fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)] mod scope_tests {use super::*;
 #[test] fn parent_paths_and_symlinks_require_approval(){
 let root=std::env::temp_dir().join(format!("quixi-scope-{}",Uuid::new_v4()));fs::create_dir_all(root.join("work")).unwrap();fs::write(root.join("outside"),"private").unwrap();let cwd=root.join("work");let cwd=cwd.to_str().unwrap();
 assert!(file_access_outside_workspace("Read",&json!({"file_path":"../outside"}),cwd));assert!(!file_access_outside_workspace("Write",&json!({"file_path":"new/nested.txt"}),cwd));
 #[cfg(unix)]{std::os::unix::fs::symlink(root.join("outside"),root.join("work/link")).unwrap();assert!(file_access_outside_workspace("Read",&json!({"file_path":"link"}),cwd));}
 fs::remove_dir_all(root).unwrap();}
}
