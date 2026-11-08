//! The messages mods send `provider:pi`, each by its one distinguishing key.
//! Anything that fails comes back as a toast in `app.pi.events`.
//!
//! What is quick happens here, in order, on the provider's loop; what waits
//! on pi or git goes to a worker. The same order the Node provider's event
//! loop gave: a message's first steps before the next message's.

use {
    crate::{
        config::real_path,
        core::Core,
        engine::{Engine, Entry, paths_of, picked_model},
        flows::{LiveRef, NewThread, pi_path, same_live},
        git,
        resources::{
            forget_key, remove_endpoint, save_endpoint, save_key, set_extension, set_skill,
        },
        rpc::{Rpc, Start},
        schedule::next_run,
        snapshot::SHOWN_ITEMS,
        store::Meta,
        value::{as_list, display, now_millis, now_seconds},
    },
    base64::Engine as _,
    serde_json::{Map, Value, json},
    std::{fs, path::Path, sync::Arc, time::Duration},
    uuid::Uuid,
};

const MAX_IMAGE: u64 = 8 * 1024 * 1024;
const COMPACT_TIMEOUT: Duration = Duration::from_secs(300);
const SHELL_TIMEOUT: Duration = Duration::from_secs(600);

/// Which key decides what a message means, first match first.
const NAMES: &[&str] = &[
    "add_workspace",
    "remove_workspace",
    "select",
    "new_thread",
    "prompt",
    "returned_taken",
    "clear_queue",
    "set_model",
    "set_thinking",
    "rename",
    "pin",
    "archive",
    "delete_thread",
    "fork",
    "expand",
    "show_more",
    "answer",
    "schedule",
    "delete_schedule",
    "run_schedule",
    "set_skill",
    "set_extension",
    "save_key",
    "forget_key",
    "save_endpoint",
    "remove_endpoint",
    "set_pref",
    "refresh",
];

fn image_type(path: &str) -> Option<&'static str> {
    let ending = Path::new(path).extension()?.to_str()?.to_lowercase();
    match ending.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn images(paths: &[String]) -> Result<Vec<Value>, String> {
    let mut list = Vec::new();
    for path in paths {
        let kind = image_type(path).ok_or(format!("pi takes pictures, and {path} is not one."))?;
        let meta = fs::metadata(path).map_err(|error| format!("{path}: {error}"))?;
        if meta.len() > MAX_IMAGE {
            return Err(format!("{path} is over 8 MB."));
        }
        let bytes = fs::read(path).map_err(|error| format!("{path}: {error}"))?;
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        list.push(json!({ "type": "image", "mimeType": kind, "data": data }));
    }
    Ok(list)
}

/// Commands pi's own terminal handles itself, done here over RPC.
fn builtin(core: &Arc<Core>, id: &str, text: &str) -> Result<bool, String> {
    let body = text.strip_prefix('/').unwrap_or(text);
    let name = body.split_whitespace().next().unwrap_or_default();
    let rest = body[name.len()..].trim_start();
    match name {
        "compact" => {
            let live = core.ensure_live(id)?;
            let fields = if rest.is_empty() {
                json!({})
            } else {
                json!({ "customInstructions": rest })
            };
            live.rpc.send_within("compact", fields, COMPACT_TIMEOUT)?;
            core.lock().load_entries(id);
            Ok(true)
        },
        "name" => {
            if !rest.is_empty() {
                let live = core.ensure_live(id)?;
                live.rpc
                    .send("set_session_name", json!({ "name": rest.trim() }))?;
            }
            Ok(true)
        },
        "reload" => {
            {
                let mut engine = core.lock();
                core.stop_live(&mut engine, id);
            }
            core.ensure_live(id)?;
            Ok(true)
        },
        _ => Ok(false),
    }
}

/// What the composer's text means: a shell line, a command, or a prompt.
fn submit(
    core: &Arc<Core>,
    id: &str,
    text: &str,
    paths: &[String],
    behavior: Option<&str>,
) -> Result<(), String> {
    let sent = (|| {
        let attached = images(paths)?;
        if text.starts_with('!') {
            return shell(core, id, text);
        }
        if text.starts_with('/') && builtin(core, id, text)? {
            return Ok(());
        }
        core.prompt(id, text, attached, behavior)
    })();
    // The composer emptied itself on sending: the words go back to this
    // thread's, whichever is open by the time a slow failure comes. Mods
    // remember the last id across launches, so a count won't do.
    if sent.is_err() && !text.is_empty() {
        let back = json!({ "text": text, "id": Uuid::new_v4().to_string() });
        core.lock().returned.insert(id.to_owned(), back);
    }
    sent
}

fn shell(core: &Arc<Core>, id: &str, text: &str) -> Result<(), String> {
    let quiet = text.starts_with("!!");
    let command = text[if quiet {
        2
    } else {
        1
    }..]
        .trim();
    let thread = core.ensure_live(id)?;
    let fields = json!({ "command": command, "excludeFromContext": quiet });
    let result = thread.rpc.send_within("bash", fields, SHELL_TIMEOUT)?;
    core.refresh(id, &thread.rpc)?;
    let mut engine = core.lock();
    let file = engine.thread(id).and_then(|meta| meta.file.clone());
    if file.is_some_and(|file| Path::new(&file).exists()) {
        engine.load_entries(id);
    } else if let Some(list) = engine.entries.get_mut(id) {
        // pi writes nothing until the model first answers, so before that
        // the output is only here. Once the file exists it holds this too.
        list.push(Entry {
            id: Some(Uuid::new_v4().to_string()),
            message: Some(json!({
                "role": "bashExecution",
                "command": command,
                "exitCode": result["exitCode"],
                "output": display(&result["output"]),
                "excludeFromContext": quiet,
            })),
            ..Entry::default()
        });
    }
    engine.changed();
    Ok(())
}

/// A pi for one job, answering nothing but what is asked of it.
fn helper(core: &Arc<Core>, args: &[String], cwd: &str) -> Result<Arc<Rpc>, String> {
    let path = pi_path(&core.lock().pi).ok_or("pi was not found.")?;
    let rpc = Rpc::spawn(Start {
        program: &path,
        args,
        cwd,
        runtime: &core.runtime(),
        on_event: Box::new(|_| {}),
        on_exit: Box::new(|_, _| {}),
    })?;
    core.keep_helper(&rpc)?;
    Ok(rpc)
}

/// A fork is a new session from an earlier prompt; a pi of its own makes it,
/// so the thread it comes from can go on running.
fn fork(core: &Arc<Core>, source: &str, entry: &Value, worktree: bool) -> Result<(), String> {
    let (file, cwd, workspace, inherited, title) = {
        let engine = core.lock();
        let meta = engine.thread(source).ok_or("That thread is gone.")?;
        let file = meta
            .file
            .clone()
            .ok_or("That thread has nothing to fork yet.")?;
        let title = format!("Fork of {}", engine.title_of(source));
        (
            file,
            meta.cwd.clone(),
            meta.workspace.clone(),
            meta.worktree.clone(),
            title,
        )
    };
    let child = helper(core, &["--session".to_owned(), file], &cwd)?;
    let forked = (|| {
        let done = child.send("fork", json!({ "entryId": entry }))?;
        if done["cancelled"].as_bool().unwrap_or(false) {
            return Ok(None);
        }
        let state = child.send("get_state", json!({}))?;
        Ok::<_, String>(Some((done, state)))
    })();
    child.stop();
    let Some((done, state)) = forked? else {
        return Ok(());
    };
    let session = display(&state["sessionFile"]);
    let mut meta = Meta {
        id: Uuid::new_v4().to_string(),
        cwd,
        workspace: workspace.clone(),
        file: Some(session.clone()),
        title: Some(title),
        worktree: inherited,
        ..Meta::default()
    };
    if worktree {
        move_to_worktree(core, &mut meta)?;
    }
    let session = meta.file.clone().unwrap_or(session);
    let own = meta.id.clone();
    {
        let mut engine = core.lock();
        engine.data_mut().threads.insert(own.clone(), meta);
        engine.store.save();
    }
    core.scan();
    let chosen = {
        let mut engine = core.lock();
        // A scan adopts the session under pi's own id: that one stays.
        let adopted = engine
            .data()
            .threads
            .values()
            .find(|item| item.file.as_deref() == Some(session.as_str()) && item.id != own)
            .map(|item| item.id.clone());
        if let Some(adopted) = &adopted {
            let made = engine.data_mut().threads.remove(&own);
            if let (Some(made), Some(item)) = (made, engine.thread_mut(adopted)) {
                item.title = made.title;
                item.worktree = made.worktree;
                item.workspace = workspace;
            }
        }
        adopted.unwrap_or(own)
    };
    core.select(Some(chosen));
    let text = display(&done["text"]);
    core.lock().prefill = Some(json!({ "text": text, "seq": now_millis() }));
    Ok(())
}

fn move_to_worktree(core: &Arc<Core>, meta: &mut Meta) -> Result<(), String> {
    let words = meta.title.clone().unwrap_or_default();
    let (path, branch) = git::add_worktree(&core.config, &core.runtime(), &meta.workspace, &words)?;
    let file = meta.file.clone().unwrap_or_default();
    let child = helper(core, &["--fork".to_owned(), file], &path)?;
    let state = child.send("get_state", json!({}));
    child.stop();
    meta.file = Some(display(&state?["sessionFile"]));
    meta.cwd.clone_from(&path);
    meta.worktree = Some(crate::store::Worktree { path, branch });
    Ok(())
}

fn save_schedule(engine: &mut Engine, task: &Map<String, Value>) {
    let now = now_seconds();
    let list = &mut engine.data_mut().schedules;
    let saved = list
        .iter()
        .position(|item| item.get("id") == task.get("id") && task.contains_key("id"));
    let defaults = json!({ "enabled": true, "cadence": "daily", "time": "09:00", "weekday": 1 });
    let mut merged = defaults.as_object().cloned().unwrap_or_default();
    if let Some(at) = saved {
        merged.extend(list[at].clone());
    }
    merged.extend(task.clone());
    merged
        .entry("id")
        .or_insert_with(|| json!(Uuid::new_v4().to_string()));
    let next = next_run(&merged, now);
    merged.insert("next".to_owned(), json!(next));
    match saved {
        Some(at) => list[at] = merged,
        None => list.push(merged),
    }
    engine.store.save();
}

fn delete_thread(core: &Arc<Core>, id: &str) -> Result<(), String> {
    let (file, worktree, workspace, heir) = {
        let mut engine = core.lock();
        let Some(meta) = engine.thread(id).cloned() else {
            return Ok(());
        };
        core.stop_live(&mut engine, id);
        // A fork can share the checkout: the last thread in it removes it.
        let heir = engine
            .data()
            .threads
            .values()
            .find(|other| other.id != id && other.cwd == meta.cwd)
            .map(|other| other.id.clone());
        (meta.file, meta.worktree, meta.workspace, heir)
    };
    if let Some(file) = file {
        let _ = fs::remove_file(file);
    }
    if let Some(worktree) = worktree {
        if let Some(heir) = heir {
            if let Some(meta) = core.lock().thread_mut(&heir) {
                meta.worktree.get_or_insert(worktree);
            }
        } else if let Err(problem) =
            git::remove_worktree(&core.runtime(), &workspace, &worktree.path)
        {
            tracing::warn!("pilot: {problem}");
        }
    }
    let mut engine = core.lock();
    engine.data_mut().threads.remove(id);
    engine.returned.remove(id);
    engine.summaries.remove(id);
    engine.entries.remove(id);
    if engine.selected() == Some(id) {
        engine.data_mut().selected = None;
        crate::value::lock(&core.urgent).selected = None;
    }
    engine.store.save();
    engine.changed();
    Ok(())
}

/// Tells a thread's running pi of a change, if it has one, then asks where
/// it stands. Written here, on the loop, so a prompt sent after the change
/// reaches pi after it; answered off the loop, as pi may take a moment.
fn tell_live(
    core: &Arc<Core>,
    live: LiveRef,
    kind: &'static str,
    fields: Value,
    then: impl FnOnce(&Arc<Core>, &LiveRef) -> Result<(), String> + Send + 'static,
) {
    let asked = live.rpc.ask(kind, fields);
    core.work(move |core| {
        live.rpc.answer_within(asked, crate::rpc::COMMAND_TIMEOUT)?;
        core.refresh(&live.id, &live.rpc)?;
        then(core, &live)
    });
}

/// The running pi of the thread a `set_` message means, its choice noted on
/// the thread with `note`. From the start page, `draft` means the next
/// thread, not the one selected.
fn selected_live(
    core: &Arc<Core>,
    message: &Map<String, Value>,
    note: impl FnOnce(&mut Meta),
) -> Option<LiveRef> {
    let mut engine = core.lock();
    if truthy(message.get("draft")) {
        engine.store.save();
        return None;
    }
    let id = engine.selected().map(str::to_owned)?;
    let meta = engine.thread_mut(&id)?;
    note(meta);
    engine.store.save();
    let live = engine.live.get(&id)?;
    Some(LiveRef {
        id,
        rpc: Arc::clone(&live.rpc),
    })
}

fn truthy(value: Option<&Value>) -> bool {
    !matches!(value, None | Some(Value::Null | Value::Bool(false)))
}

fn new_thread(core: &Arc<Core>, options: &Value) {
    let (options, paths) = (options.clone(), paths_of(&options["images"]));
    core.work(move |core| {
        let last = core.lock().data().prefs.get("last_model").cloned();
        let text = |key: &str| options[key].as_str().map(str::to_owned);
        let prompt = text("prompt");
        let words = prompt.clone().unwrap_or_default();
        let new = NewThread {
            workspace: text("workspace"),
            worktree: options["mode"] == "worktree",
            prompt,
            title: text("title"),
            select: options["select"] != false,
            model: picked_model(&options["model"]).or(last),
            thinking: text("thinking"),
        };
        let files = paths.clone();
        core.create_thread(new, move |core, id| {
            if words.is_empty() && files.is_empty() {
                return Ok(());
            }
            submit(core, id, &words, &files, None)
        })
        .map(|_| ())
    });
}

fn prompt(core: &Arc<Core>, message: &Map<String, Value>) -> Result<(), String> {
    let id = message
        .get("thread")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let id = id.or_else(|| core.lock().selected().map(str::to_owned));
    let id = id.ok_or("No thread is open.")?;
    let text = message.get("prompt").map(display).unwrap_or_default();
    let paths = message.get("images").map(paths_of).unwrap_or_default();
    let behavior = message
        .get("behavior")
        .and_then(Value::as_str)
        .map(str::to_owned);
    core.work(move |core| submit(core, &id, &text, &paths, behavior.as_deref()));
    Ok(())
}

fn set_model(core: &Arc<Core>, message: &Map<String, Value>) {
    let chosen = &message["set_model"];
    let model = json!({ "provider": chosen["provider"], "id": chosen["id"] });
    core.lock()
        .data_mut()
        .prefs
        .insert("last_model".to_owned(), model.clone());
    let live = selected_live(core, message, |meta| meta.model = Some(model));
    if let Some(live) = live {
        let fields = json!({ "provider": chosen["provider"], "modelId": chosen["id"] });
        tell_live(core, live, "set_model", fields, |core, live| {
            let levels = live.rpc.send("get_available_thinking_levels", json!({}))?;
            let mut engine = core.lock();
            if let Some(live) = same_live(&mut engine, &live.id, &live.rpc) {
                live.levels = as_list(&levels["levels"]).to_vec();
            }
            Ok(())
        });
    }
}

fn set_thinking(core: &Arc<Core>, message: &Map<String, Value>) {
    let level = message["set_thinking"].clone();
    core.lock()
        .data_mut()
        .prefs
        .insert("last_thinking".to_owned(), level.clone());
    let chosen = level.as_str().map(str::to_owned);
    let live = selected_live(core, message, |meta| meta.thinking = chosen);
    if let Some(live) = live {
        let fields = json!({ "level": level });
        tell_live(core, live, "set_thinking_level", fields, |_, _| Ok(()));
    }
}

fn answer(core: &Core, value: &Value) {
    let Value::Object(fields) = value else {
        return;
    };
    let mut fields = fields.clone();
    let dialog = fields.remove("dialog").unwrap_or(Value::Null);
    let mut engine = core.lock();
    let id = engine.selected().map(str::to_owned);
    if let Some(live) = id.and_then(|id| engine.live.get_mut(&id)) {
        live.answer(&dialog, fields);
    }
}

fn flag(core: &Core, value: &Value, set: impl FnOnce(&mut Meta, bool)) {
    let mut engine = core.lock();
    let on = value["on"].as_bool().unwrap_or(false);
    if let Some(meta) = value["id"].as_str().and_then(|id| engine.thread_mut(id)) {
        set(meta, on);
        engine.store.save();
    }
}

fn toggled_on_selected(core: &Core, tool: &Value) {
    let mut engine = core.lock();
    if let Some(id) = engine.selected().map(str::to_owned) {
        let open = engine.expanded.entry(id).or_default();
        let tool = display(tool);
        if !open.remove(&tool) {
            open.insert(tool);
        }
    }
}

fn run_one(core: &Arc<Core>, name: &str, message: &Map<String, Value>) -> Result<(), String> {
    let value = &message[name];
    let config = Arc::clone(&core.config);
    let text = display(value);
    match name {
        "add_workspace" => {
            let real = real_path(&text)?;
            core.lock().add_workspace(&real);
            core.refresh_branch(real);
            core.scan();
        },
        "remove_workspace" => {
            {
                let mut engine = core.lock();
                engine
                    .data_mut()
                    .workspaces
                    .retain(|item| item.path != text);
                let doomed: Vec<String> = engine
                    .data()
                    .threads
                    .values()
                    .filter(|meta| meta.workspace == text)
                    .map(|meta| meta.id.clone())
                    .collect();
                for id in doomed {
                    core.stop_live(&mut engine, &id);
                    engine.data_mut().threads.remove(&id);
                }
                engine.store.save();
            }
            core.scan();
        },
        "select" => core.select(value.as_str().map(str::to_owned)),
        "new_thread" => new_thread(core, value),
        "prompt" => prompt(core, message)?,
        "returned_taken" => {
            let mut engine = core.lock();
            let thread = display(&value["thread"]);
            let same = engine
                .returned
                .get(&thread)
                .is_some_and(|back| back["id"] == value["id"]);
            if same {
                engine.returned.remove(&thread);
            }
        },
        "clear_queue" => {
            let engine = core.lock();
            let live = engine.selected().and_then(|id| engine.live.get(id));
            if let Some(rpc) = live.map(|live| Arc::clone(&live.rpc)) {
                drop(engine);
                core.work(move |_| rpc.send("clear_queue", json!({})).map(|_| ()));
            }
        },
        "set_model" => set_model(core, message),
        "set_thinking" => set_thinking(core, message),
        "rename" => {
            let (id, title) = (display(&value["id"]), value["title"].clone());
            core.work(move |core| {
                let live = core.ensure_live(&id)?;
                live.rpc
                    .send("set_session_name", json!({ "name": title }))
                    .map(|_| ())
            });
        },
        "pin" => flag(core, value, |meta, on| meta.pinned = on),
        "archive" => flag(core, value, |meta, on| meta.archived = on),
        "delete_thread" => core.work(move |core| delete_thread(core, &text)),
        "fork" => {
            let (id, entry) = (display(&value["thread"]), value["entry"].clone());
            let worktree = truthy(value.get("worktree"));
            core.work(move |core| fork(core, &id, &entry, worktree));
        },
        "expand" => toggled_on_selected(core, value),
        "show_more" => {
            let mut engine = core.lock();
            if let Some(id) = engine.selected().map(str::to_owned) {
                let shown = engine.shown.entry(id).or_insert(SHOWN_ITEMS);
                *shown += SHOWN_ITEMS;
            }
        },
        "answer" => answer(core, value),
        "schedule" => {
            if let Value::Object(task) = value {
                save_schedule(&mut core.lock(), task);
            }
        },
        "delete_schedule" => {
            let mut engine = core.lock();
            engine
                .data_mut()
                .schedules
                .retain(|task| task.get("id") != Some(value));
            engine.store.save();
        },
        "run_schedule" => core.work(move |core| core.run_task(&text)),
        "set_skill" => {
            let on = value["enabled"].as_bool().unwrap_or(false);
            set_skill(&config, &display(&value["id"]), on)?;
            core.config_changed();
        },
        "set_extension" => {
            let on = value["enabled"].as_bool().unwrap_or(false);
            set_extension(&config, &display(&value["id"]), on)?;
            core.config_changed();
        },
        "save_key" => save_key(
            &config,
            &display(&value["provider"]),
            &display(&value["key"]),
        )?,
        "forget_key" => forget_key(&config, &text)?,
        "save_endpoint" => save_endpoint(&config, value)?,
        "remove_endpoint" => remove_endpoint(&config, &text)?,
        "set_pref" => {
            if let Value::Object(prefs) = value {
                let mut engine = core.lock();
                engine.data_mut().prefs.extend(prefs.clone());
                engine.store.save();
            }
        },
        "refresh" => {
            core.lock().resources_asked += 1;
            core.scan();
        },
        _ => {},
    }
    Ok(())
}

/// Acts on a message from a mod.
pub(crate) fn handle(core: &Arc<Core>, message: &Value) {
    let Value::Object(fields) = message else {
        return;
    };
    let Some(name) = NAMES.iter().find(|name| fields.contains_key(**name)) else {
        return;
    };
    if let Err(problem) = run_one(core, name, fields) {
        core.fail(&problem, None);
    }
    core.changed();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_needs_a_picture_s_name() {
        let refused = images(&["notes.txt".to_owned()]);
        assert!(refused.is_err_and(|problem| problem.contains("is not one")));
        assert_eq!(image_type("A.JPG"), Some("image/jpeg"));
    }
}
