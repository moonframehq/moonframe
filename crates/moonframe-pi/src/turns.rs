//! A run from its prompt reaching pi to its end: what it changed, what the
//! window hears of it, and a first run's title. Each step waits on git or
//! pi, on the thread's own queue, so pi's events go on arriving meanwhile.

use {
    crate::{
        core::Core,
        flows::{pi_path, same_live},
        git,
        rpc::Rpc,
        run::run,
        value::{as_list, display, first_chars},
    },
    serde_json::{Value, json},
    std::{sync::Arc, time::Duration},
};

const TITLE_TIMEOUT: Duration = Duration::from_secs(45);

impl Core {
    /// A run's "before" is taken ahead of sending its prompt, as a quick run
    /// could otherwise edit first. A queued prompt has no such moment: its
    /// start is the end of the run before it, and the best there is.
    pub(crate) fn turn_started(&self, id: &str, rpc: &Arc<Rpc>, message: &Value) {
        let (ready, cwd) = {
            let mut engine = self.lock();
            let Some(live) = same_live(&mut engine, id, rpc) else {
                return;
            };
            (live.next_before.take(), live.cwd.clone())
        };
        let before = ready.or_else(|| git::snapshot(&self.runtime(), &cwd));
        self.close_turn(id, rpc, before.as_deref());
        let mut engine = self.lock();
        if let Some(live) = same_live(&mut engine, id, rpc) {
            live.turn = Some(display(&message["timestamp"]));
            live.before = before;
        }
    }

    fn close_turn(&self, id: &str, rpc: &Arc<Rpc>, after: Option<&str>) {
        let Some(after) = after else {
            return;
        };
        let (key, before, cwd) = {
            let mut engine = self.lock();
            let Some(live) = same_live(&mut engine, id, rpc) else {
                return;
            };
            let (Some(_), Some(before)) = (&live.turn, live.before.clone()) else {
                return;
            };
            let Some(key) = live.turn.take() else {
                return;
            };
            (key, before, live.cwd.clone())
        };
        if before == after {
            return;
        }
        let changes = git::changes_between(&self.runtime(), &cwd, &before, after);
        let Ok(mut turn) = changes else {
            return;
        };
        if as_list(&turn["files"]).is_empty() {
            return;
        }
        turn["before"] = json!(before);
        turn["after"] = json!(after);
        self.lock().record_turn(id, key, turn);
    }

    pub(crate) fn settled(self: &Arc<Self>, id: &str, rpc: &Arc<Rpc>, aborted: bool) {
        let cwd = {
            let mut engine = self.lock();
            if engine.thread(id).is_none() {
                return;
            }
            let Some(live) = same_live(&mut engine, id, rpc) else {
                return;
            };
            live.cwd.clone()
        };
        let after = git::snapshot(&self.runtime(), &cwd);
        self.close_turn(id, rpc, after.as_deref());
        // A pi that has gone says nothing; the file still has the run.
        let _ = self.refresh(id, rpc);
        {
            let mut engine = self.lock();
            let file = engine.live.get(id).and_then(|live| {
                live.state.as_ref()?["sessionFile"]
                    .as_str()
                    .map(str::to_owned)
            });
            if let (Some(file), Some(meta)) = (file, engine.thread_mut(id)) {
                meta.file = Some(file);
            }
            engine.load_entries(id);
        }
        self.refresh_summary(id);
        {
            let mut engine = self.lock();
            let last = engine.last_answer(id).cloned();
            let failed = last
                .as_ref()
                .is_some_and(|last| last["stopReason"] == "error");
            let title = engine.title_of(id);
            if failed {
                engine.mark_failed(id);
                let body = last.as_ref().and_then(|last| last["errorMessage"].as_str());
                engine.emit("failed", Some(id), &title, body.unwrap_or_default(), "info");
            } else if !aborted {
                if engine.selected() != Some(id)
                    && let Some(meta) = engine.thread_mut(id)
                {
                    meta.unread = true;
                }
                let preview = engine.summaries.get(id).map(|info| info.preview.clone());
                let preview = preview.unwrap_or_default();
                engine.emit("finished", Some(id), &title, &preview, "info");
            }
            engine.store.save();
            engine.changed();
        }
        let (thread, rpc) = (id.to_owned(), Arc::clone(rpc));
        self.work(move |core| core.name_thread(&thread, &rpc));
    }

    /// A first run's thread gets a short title from its own model.
    fn name_thread(&self, id: &str, rpc: &Arc<Rpc>) -> Result<(), String> {
        let (path, model, cwd, words) = {
            let mut engine = self.lock();
            let auto = engine.data().prefs.get("auto_title") == Some(&json!(true));
            let info = engine.summaries.get(id);
            let fresh = info.is_some_and(|info| {
                info.name.is_none()
                    && info
                        .prompt
                        .as_ref()
                        .is_some_and(|prompt| !prompt.is_empty())
                    && info.messages <= 1
            });
            if !auto || !fresh {
                return Ok(());
            }
            let words = engine.first_prompt(id).cloned().unwrap_or(Value::Null);
            let path = pi_path(&engine.pi);
            let live = same_live(&mut engine, id, rpc);
            let state = live.as_ref().and_then(|live| live.state.as_ref());
            let model = state
                .map(|state| &state["model"])
                .filter(|model| model.is_object());
            let model = model
                .map(|model| format!("{}/{}", display(&model["provider"]), display(&model["id"])));
            let cwd = live.map(|live| live.cwd.clone());
            let (Some(path), Some(model), Some(cwd)) = (path, model, cwd) else {
                return Ok(());
            };
            (path, model, cwd, words)
        };
        let asked = first_chars(&words.to_string(), 1500).to_owned();
        let prompt = format!(
            "Write a short title of at most five words for this request. Answer with the title \
             alone.\n\n{asked}"
        );
        let mut command = self.runtime().command(path);
        command.current_dir(cwd).args([
            "-p",
            "--no-session",
            "--no-tools",
            "--no-extensions",
            "--no-skills",
            "--no-context-files",
            "--no-prompt-templates",
            "--thinking",
            "off",
            "--model",
            &model,
            &prompt,
        ]);
        // `pi -p` reads piped stdin as part of the prompt, to its end.
        let Ok(out) = run(command, None, TITLE_TIMEOUT).output() else {
            return Ok(());
        };
        let clean = clean_title(out.trim().lines().next().unwrap_or_default());
        if clean.is_empty() || !self.same_rpc(id, rpc) {
            return Ok(());
        }
        rpc.send("set_session_name", json!({ "name": clean }))
            .map(|_| ())
    }

    fn same_rpc(&self, id: &str, rpc: &Arc<Rpc>) -> bool {
        same_live(&mut self.lock(), id, rpc).is_some()
    }
}

/// A model's answer as a title: no quotes, marks or trailing stop.
fn clean_title(title: &str) -> String {
    let start = title.trim_start_matches(|c: char| "\"'#".contains(c) || c.is_whitespace());
    let end = start.trim_end_matches(|c: char| "\"'.".contains(c) || c.is_whitespace());
    first_chars(end, 60).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_loses_its_quotes_and_stop() {
        assert_eq!(clean_title("\"Fix The Price.\""), "Fix The Price");
        assert_eq!(clean_title("# Tidy"), "Tidy");
    }
}
