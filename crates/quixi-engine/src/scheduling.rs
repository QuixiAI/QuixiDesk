//! Persisted, at-most-once desktop scheduling. Missed times are reported, not replayed.
use super::*;
use std::str::FromStr;
pub(crate) fn next_run(expression: &str, after: i64) -> Result<i64, String> {
    let fields: Vec<_> = expression.split_whitespace().collect();
    if fields.len() != 5 {
        return Err("Use five cron fields: minute hour day month weekday.".into());
    }
    let mut days = std::collections::BTreeSet::new();
    let day_number = |s: &str| -> Result<u32, String> {
        match s.to_uppercase().as_str() {
            "SUN" => Ok(0),
            "MON" => Ok(1),
            "TUE" => Ok(2),
            "WED" => Ok(3),
            "THU" => Ok(4),
            "FRI" => Ok(5),
            "SAT" => Ok(6),
            _ => s
                .parse::<u32>()
                .ok()
                .filter(|n| *n <= 7)
                .ok_or("Invalid weekday".into()),
        }
    };
    for item in fields[4].split(',') {
        let parts: Vec<_> = item.split('/').collect();
        if parts.len() > 2 {
            return Err("Invalid weekday step".into());
        }
        let step = if parts.len() == 2 {
            parts[1]
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or("Invalid weekday step")?
        } else {
            1
        };
        let (start, end) = if parts[0] == "*" {
            (0, 6)
        } else if let Some((a, b)) = parts[0].split_once('-') {
            (day_number(a)?, day_number(b)?)
        } else {
            let n = day_number(parts[0])?;
            (n, if parts.len() == 2 { 7 } else { n })
        };
        if start > end {
            return Err("Weekday range must run forwards".into());
        }
        for n in (start..=end).step_by(step) {
            days.insert(n % 7 + 1);
        }
    }
    let weekday = days
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let parse = |day: &str, week: &str| {
        cron::Schedule::from_str(&format!(
            "0 {} {} {} {} {}",
            fields[0], fields[1], day, fields[3], week
        ))
        .map_err(|e| format!("Invalid schedule: {e}"))
    };
    let now = chrono::DateTime::from_timestamp_millis(after)
        .ok_or("Invalid schedule time")?
        .with_timezone(&chrono::Local);
    let next = |schedule: cron::Schedule| schedule.after(&now).next().map(|t| t.timestamp_millis());
    let result = if fields[2] != "*" && fields[4] != "*" {
        [next(parse(fields[2], "*")?), next(parse("*", &weekday)?)]
            .into_iter()
            .flatten()
            .min()
    } else {
        next(parse(fields[2], &weekday)?)
    };
    result.ok_or("This schedule has no future occurrence.".into())
}
impl QuixiEngine {
    pub fn preview_schedule(&self, expression: &str) -> Value {
        let mut after = Utc::now().timestamp_millis();
        let mut runs = Vec::new();
        for _ in 0..3 {
            match next_run(expression, after) {
                Ok(next) => {
                    runs.push(next);
                    after = next;
                }
                Err(error) => return json!({"error":error}),
            }
        }
        json!({"runs":runs,"timezone":chrono::Local::now().format("%Z").to_string(),"missedRunPolicy":"skip"})
    }

    /// Reconcile run state with its conversation. "Finished" means the turn ended,
    /// not a guarantee that the model accomplished the user's requested work.
    pub fn scheduled_task_list(&self) -> Vec<Value> {
        let _guard = self.schedule_dispatch.lock();
        let mut tasks = self.scheduled_tasks.write();
        let sessions = self.agent_sessions.read();
        for (id, task) in tasks.iter_mut() {
            if !matches!(
                task["lastRunStatus"].as_str(),
                Some("started" | "running" | "waiting_approval")
            ) {
                continue;
            }
            let sid = task["lastSessionId"].as_str().unwrap_or_default();
            let (status, message) = match sessions.get(sid) {
                None => (
                    "interrupted",
                    "The run's conversation is unavailable. Run again to retry.".to_string(),
                ),
                Some(s) if s.error.is_some() => ("failed", s.error.clone().unwrap_or_default()),
                Some(s) if s.is_running && !s.pending_tool_permissions.is_empty() => (
                    "waiting_approval",
                    "Open the run to approve or decline its requested action.".to_string(),
                ),
                Some(s) if s.is_running => ("running", "The conversation is running.".to_string()),
                Some(s) => {
                    let result = s
                        .buffered_messages
                        .iter()
                        .rev()
                        .find(|m| m["type"] == "result");
                    if result.is_some_and(|r| r["is_error"] == true) {
                        (
                            "failed",
                            "The run reported an error. Open the conversation for details."
                                .to_string(),
                        )
                    } else if result.is_some() {
                        (
                            "finished",
                            "The run finished. Open its conversation to review the result."
                                .to_string(),
                        )
                    } else {
                        ("interrupted", "The run stopped before a final result. Open the conversation or run again.".to_string())
                    }
                }
            };
            if task["lastRunStatus"] == status && task["lastRunMessage"] == message {
                continue;
            }
            let mut updated = task.clone();
            updated["lastRunStatus"] = json!(status);
            updated["lastRunMessage"] = json!(message);
            if matches!(status, "finished" | "failed" | "interrupted") {
                updated["lastFinishedAt"] = json!(Utc::now().timestamp_millis());
            }
            let path = self
                .data_dir
                .join("scheduled_tasks")
                .join(format!("{id}.json"));
            if storage::atomic_write(&path, &serde_json::to_vec_pretty(&updated).unwrap()).is_ok() {
                *task = updated;
            }
        }
        let mut rows: Vec<_> = tasks
            .values()
            .filter(|task| task["status"] != "deleted")
            .cloned()
            .collect();
        rows.sort_by_key(|task| {
            task["displayName"]
                .as_str()
                .unwrap_or_default()
                .to_lowercase()
        });
        rows
    }

    pub(crate) fn start_scheduler(self: &Arc<Self>) {
        if cfg!(test) {
            return;
        }
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let Some(engine) = weak.upgrade() else {
                break;
            };
            engine.dispatch_due_schedules(Utc::now().timestamp_millis());
        });
    }
    pub fn run_scheduled_task(self: &Arc<Self>, id: &str) -> Value {
        self.dispatch_schedule(id, Utc::now().timestamp_millis(), false)
    }
    pub(crate) fn dispatch_due_schedules(self: &Arc<Self>, now: i64) {
        self.scheduled_task_list();
        let ids: Vec<String> = self
            .scheduled_tasks
            .read()
            .iter()
            .filter(|(_, t)| {
                t["enabled"] == true
                    && t["dispatchVersion"] == 1
                    && t["nextRunAt"].as_i64().is_some_and(|due| due <= now)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.dispatch_schedule(&id, now, true);
        }
    }
    fn dispatch_schedule(self: &Arc<Self>, id: &str, now: i64, automatic: bool) -> Value {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return json!({"error":"Invalid schedule identifier."});
        }
        let _guard = self.schedule_dispatch.lock();
        let Some(mut task) = self.scheduled_tasks.read().get(id).cloned() else {
            return json!({"error":"Schedule not found."});
        };
        if task["status"] == "deleted" {
            return json!({"error":"Schedule was deleted."});
        }
        let due = task["nextRunAt"].as_i64().unwrap_or(i64::MAX);
        if automatic && (task["enabled"] != true || task["dispatchVersion"] != 1 || due > now) {
            return json!({"skipped":true});
        }
        let busy = task["lastSessionId"].as_str().is_some_and(|sid| {
            self.agent_sessions
                .read()
                .get(sid)
                .is_some_and(|s| s.is_running)
        });
        let problem = if busy {
            Some("Previous run is still active; this occurrence was skipped.")
        } else if automatic && now.saturating_sub(due) > 60_000 {
            Some("Scheduled time was missed while the app was closed or asleep. No catch-up task was run.")
        } else if self.provider_manager.get_public_state()["hasConfiguredProvider"] != true {
            Some("No provider is configured.")
        } else if task["model"].as_str().is_some_and(|model| {
            !model.is_empty() && self.provider_manager.resolve_connected_model(Some(model)) != model
        }) {
            Some("The selected model is no longer available. Edit the schedule to choose another model.")
        } else if task["spaceId"]
            .as_str()
            .is_some_and(|id| !self.cowork_spaces.read().contains_key(id))
        {
            Some("The associated project is unavailable. Edit this schedule to choose another project.")
        } else if task["cwd"]
            .as_str()
            .is_some_and(|cwd| !cwd.is_empty() && !Path::new(cwd).is_dir())
        {
            Some("Workspace folder is unavailable.")
        } else {
            None
        };
        // A manual retry must not move the next scheduled occurrence.
        if automatic {
            task["nextRunAt"] = next_run(task["cronExpression"].as_str().unwrap_or_default(), now)
                .map_or(Value::Null, |t| json!(t));
        }
        task["lastAttemptAt"] = json!(now);
        let sid = format!("local_{}", Uuid::new_v4());
        if let Some(problem) = problem {
            task["lastRunStatus"] = json!(if automatic { "skipped" } else { "blocked" });
            task["lastRunMessage"] = json!(problem);
        } else {
            task["lastSessionId"] = json!(sid);
            task["lastRunStatus"] = json!("started");
            task["lastRunMessage"] =
                json!("Open the conversation to follow progress or approve tools.");
        }
        let path = self
            .data_dir
            .join("scheduled_tasks")
            .join(format!("{id}.json"));
        if let Err(e) = storage::atomic_write(&path, &serde_json::to_vec_pretty(&task).unwrap()) {
            return json!({"error":format!("Schedule was not started because its state could not be saved: {e}")});
        }
        self.scheduled_tasks.write().insert(id.into(), task.clone());
        if problem.is_none() {
            self.start_agent_session(&json!({"sessionId":sid,"title":task["displayName"],"message":task["prompt"],"cwd":task["cwd"],"userSelectedFolders":task["userSelectedFolders"],"spaceId":task["spaceId"],"model":task["model"],"permissionMode":task["permissionMode"]}));
        }
        self.emit_eipc(
            "quixi.web",
            "CoworkScheduledTasks",
            "onScheduledTaskEvent",
            json!({"type":"scheduled_task_updated","scheduledTask":task}),
        );
        if !automatic && problem.is_some() {
            json!({"error":problem})
        } else if problem.is_some() {
            json!({"skipped":true})
        } else {
            json!({"sessionId":sid})
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn isolated_engine() -> (Arc<QuixiEngine>, PathBuf) {
        let root = std::env::temp_dir().join(format!("quixi-schedule-{}", Uuid::new_v4()));
        let engine = QuixiEngine::new_with_data_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
            root.clone(),
        );
        (engine, root)
    }
    #[test]
    fn previews_use_the_same_cron_engine_and_explicit_skip_policy() {
        let (e, root) = isolated_engine();
        let preview = e.preview_schedule("0 9 * * 1-5");
        let runs = preview["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 3);
        assert_eq!(preview["missedRunPolicy"], "skip");
        for pair in runs.windows(2) {
            assert_eq!(
                next_run("0 9 * * 1-5", pair[0].as_i64().unwrap()).unwrap(),
                pair[1].as_i64().unwrap()
            );
        }
        assert!(e.preview_schedule("90 9 * * *")["error"].is_string());
        drop(e);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn identical_names_create_distinct_tasks_and_folderless_tasks_stay_folderless() {
        let (e, root) = isolated_engine();
        let first = e.create_or_update_scheduled_task(
            &json!({"name":"Briefing", "prompt":"First", "cwd":""}),
        );
        let second = e.create_or_update_scheduled_task(
            &json!({"name":"Briefing", "prompt":"Second", "cwd":""}),
        );
        assert_ne!(first["id"], second["id"]);
        assert_eq!(first["cwd"], "");
        assert_eq!(first["userSelectedFolders"], json!([]));
        assert_eq!(
            e.scheduled_tasks.read()[first["id"].as_str().unwrap()]["prompt"],
            "First"
        );
        assert!(
            e.create_or_update_scheduled_task(&json!({"name":"Bad", "spaceId":"missing"}))["error"]
                .is_string()
        );
        assert!(e.create_or_update_scheduled_task(
            &json!({"name":"Bad", "permissionMode":"bypassPermissions"})
        )["error"]
            .is_string());
        drop(e);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn blocked_manual_retry_preserves_next_occurrence_and_persists_reason() {
        let (e, root) = isolated_engine();
        let task = e.create_or_update_scheduled_task(&json!({"name":"Manual retry", "cwd":""}));
        let id = task["id"].as_str().unwrap();
        let before = task["nextRunAt"].clone();
        // A removed folder deterministically blocks without sending a provider request.
        e.scheduled_tasks.write().get_mut(id).unwrap()["cwd"] = json!(root.join("removed-folder"));
        assert!(e.run_scheduled_task(id)["error"].is_string());
        let after = e.scheduled_tasks.read()[id].clone();
        assert_eq!(after["nextRunAt"], before);
        assert_eq!(after["lastRunStatus"], "blocked");
        let disk: Value = serde_json::from_slice(
            &fs::read(root.join("scheduled_tasks").join(format!("{id}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(disk["lastRunStatus"], "blocked");
        assert!(e.agent_sessions.read().is_empty());
        drop(e);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn schedule_run_state_distinguishes_approval_failure_and_interruption() {
        let (e, root) = isolated_engine();
        let task = e.create_or_update_scheduled_task(&json!({"name":"State", "cwd":""}));
        let id = task["id"].as_str().unwrap();
        let sid = e.start_agent_session(&json!({"cwd":"", "userSelectedFolders":[], "message":""}));
        {
            let mut tasks = e.scheduled_tasks.write();
            let t = tasks.get_mut(id).unwrap();
            t["lastSessionId"] = json!(sid);
            t["lastRunStatus"] = json!("started");
        }
        {
            let mut sessions = e.agent_sessions.write();
            let s = sessions.get_mut(&sid).unwrap();
            s.is_running = true;
            s.pending_tool_permissions = vec![json!({"tool":"Write"})];
        }
        assert_eq!(
            e.scheduled_task_list()[0]["lastRunStatus"],
            "waiting_approval"
        );
        {
            let mut sessions = e.agent_sessions.write();
            let s = sessions.get_mut(&sid).unwrap();
            s.is_running = false;
            s.error = Some("Provider unavailable".into());
        }
        assert_eq!(e.scheduled_task_list()[0]["lastRunStatus"], "failed");
        e.scheduled_tasks.write().get_mut(id).unwrap()["lastRunStatus"] = json!("started");
        e.agent_sessions.write().get_mut(&sid).unwrap().error = None;
        assert_eq!(e.scheduled_task_list()[0]["lastRunStatus"], "interrupted");
        drop(e);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cron_validation_and_next_occurrence() {
        let t = Utc::now().timestamp_millis();
        let n = next_run("*/5 * * * *", t).unwrap();
        assert!(n > t && n <= t + 300000);
        assert!(next_run("90 * * * *", t).is_err());
        assert!(next_run("0 9 * * MON-FRI", t).is_ok());
        assert_eq!(
            next_run("0 9 * * MON-FRI", t).unwrap(),
            next_run("0 9 * * 1-5", t).unwrap()
        );
    }
    #[test]
    fn missed_runs_advance_once_without_starting_work() {
        let root = std::env::temp_dir().join(format!("quixi-schedule-{}", Uuid::new_v4()));
        let e = QuixiEngine::new_with_data_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
            root.clone(),
        );
        let task = e.create_or_update_scheduled_task(
            &json!({"name":"Missed","cronExpression":"* * * * *","cwd":root}),
        );
        let id = task["id"].as_str().unwrap();
        let now = Utc::now().timestamp_millis();
        e.scheduled_tasks.write().get_mut(id).unwrap()["nextRunAt"] = json!(now - 120000);
        e.dispatch_due_schedules(now);
        let after = e.scheduled_tasks.read()[id].clone();
        assert_eq!(after["lastRunStatus"], "skipped");
        assert!(after["nextRunAt"].as_i64().unwrap() > now);
        assert!(e.agent_sessions.read().is_empty());
        e.dispatch_due_schedules(now);
        assert_eq!(e.scheduled_tasks.read()[id], after);
        drop(e);
        fs::remove_dir_all(root).unwrap();
    }
}
