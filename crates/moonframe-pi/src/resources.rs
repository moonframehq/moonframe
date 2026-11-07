//! pi's own configuration under its agent folder: skills, extensions, model
//! provider sign-ins and custom endpoints. Reads and writes the same files
//! the pi CLI does, so a change here is a change there.

use {
    crate::{
        Config,
        config::base_name,
        runtime::Runtime,
        value::{as_list, clip},
    },
    serde_json::{Map, Value, json},
    std::{
        fs, io,
        path::{Path, PathBuf},
        time::UNIX_EPOCH,
    },
};

/// `(id, name, env, oauth, keyless, icon)`: the providers offered in
/// Settings. `oauth` ones also have a browser sign-in, which only pi's own
/// `/login` can run.
type Offered = (
    &'static str,
    &'static str,
    Option<&'static str>,
    bool,
    bool,
    &'static str,
);

const PROVIDERS: [Offered; 10] = [
    (
        "anthropic",
        "Anthropic",
        Some("ANTHROPIC_API_KEY"),
        true,
        false,
        "brand-anthropic",
    ),
    (
        "openai",
        "OpenAI",
        Some("OPENAI_API_KEY"),
        false,
        false,
        "brand-openai",
    ),
    (
        "openai-codex",
        "ChatGPT (Codex)",
        None,
        true,
        true,
        "brand-openai",
    ),
    (
        "google",
        "Google Gemini",
        Some("GEMINI_API_KEY"),
        false,
        false,
        "agent-gemini",
    ),
    (
        "github-copilot",
        "GitHub Copilot",
        Some("COPILOT_GITHUB_TOKEN"),
        true,
        false,
        "agent-copilot",
    ),
    (
        "openrouter",
        "OpenRouter",
        Some("OPENROUTER_API_KEY"),
        false,
        false,
        "network",
    ),
    (
        "xai",
        "xAI",
        Some("XAI_API_KEY"),
        false,
        false,
        "agent-grok",
    ),
    (
        "mistral",
        "Mistral",
        Some("MISTRAL_API_KEY"),
        false,
        false,
        "brand-mistral",
    ),
    ("groq", "Groq", Some("GROQ_API_KEY"), false, false, "zap"),
    (
        "deepseek",
        "DeepSeek",
        Some("DEEPSEEK_API_KEY"),
        false,
        false,
        "sparkles",
    ),
];

const BUILTIN_EXTENSIONS: [(&str, &str, &str); 4] = [
    (
        "builtin:mcp",
        "MCP",
        "Connects the MCP servers in your configuration.",
    ),
    (
        "builtin:llama.cpp",
        "llama.cpp",
        "Runs local models through a llama.cpp router.",
    ),
    (
        "builtin:codemode",
        "Code mode",
        "Lets the model call tools by writing code.",
    ),
    (
        "builtin:tool-search",
        "Tool search",
        "Finds tools on demand when there are many.",
    ),
];

fn settings_file(config: &Config) -> PathBuf {
    config.agent_dir.join("settings.json")
}

fn auth_file(config: &Config) -> PathBuf {
    config.agent_dir.join("auth.json")
}

fn models_file(config: &Config) -> PathBuf {
    config.agent_dir.join("models.json")
}

fn read_json(file: &Path) -> Result<Value, String> {
    let name = base_name(&file.to_string_lossy());
    match fs::read_to_string(file) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|error| format!("{name} cannot be read: {error}"))
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(json!({})),
        Err(error) => Err(format!("{name} cannot be read: {error}")),
    }
}

fn write_json(file: &Path, value: &Value, private: bool) -> Result<(), String> {
    let write = || -> io::Result<()> {
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir)?;
        }
        let name = base_name(&file.to_string_lossy());
        let scratch = file.with_file_name(format!("{name}.{}", std::process::id()));
        let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
        write_new(&scratch, format!("{text}\n").as_bytes(), private)?;
        fs::rename(&scratch, file)?;
        restrict(file, private)
    };
    write().map_err(|error| error.to_string())
}

/// Writes `file`, private from its first byte if it holds secrets.
#[cfg(unix)]
fn write_new(file: &Path, bytes: &[u8], private: bool) -> io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    if private {
        options.mode(0o600);
    }
    options.open(file)?.write_all(bytes)
}

#[cfg(not(unix))]
fn write_new(file: &Path, bytes: &[u8], _private: bool) -> io::Result<()> {
    fs::write(file, bytes)
}

#[cfg(unix)]
fn restrict(file: &Path, private: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if private {
        fs::set_permissions(file, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn restrict(_file: &Path, _private: bool) -> io::Result<()> {
    Ok(())
}

/// The `key: value` lines between a file's `---` marks.
fn frontmatter(file: &Path) -> Map<String, Value> {
    let mut fields = Map::new();
    let Ok(text) = fs::read_to_string(file) else {
        return fields;
    };
    let Some(body) = text.strip_prefix("---\n") else {
        return fields;
    };
    let Some(end) = body.find("\n---") else {
        return fields;
    };
    for line in body[..end].lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphabetic() || c == '-') {
            continue;
        }
        let value = value.trim_start();
        let value = value.strip_prefix(['"', '\'']).unwrap_or(value);
        let value = value.strip_suffix(['"', '\'']).unwrap_or(value);
        fields.insert(key.to_owned(), json!(value));
    }
    fields
}

fn directories(folder: &Path) -> Vec<PathBuf> {
    let Ok(names) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut list: Vec<PathBuf> = names.flatten().map(|entry| entry.path()).collect();
    list.sort();
    list
}

fn excluded(list: &Value, id: &str) -> bool {
    let wanted = format!("-{id}");
    as_list(list).iter().any(|entry| entry == wanted.as_str())
}

fn toggled(list: &Value, id: &str, enabled: bool) -> Value {
    let (off, on) = (format!("-{id}"), format!("+{id}"));
    let mut rest: Vec<Value> = as_list(list)
        .iter()
        .filter(|entry| **entry != off.as_str() && **entry != on.as_str())
        .cloned()
        .collect();
    if !enabled {
        rest.push(json!(off));
    }
    Value::Array(rest)
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn list_skills(config: &Config, cwd: Option<&str>) -> Result<Vec<Value>, String> {
    let settings = read_json(&settings_file(config))?;
    let mut places = vec![
        (config.agent_dir.join("skills"), "user"),
        (config.home.join(".agents/skills"), "user"),
    ];
    if let Some(cwd) = cwd {
        places.push((Path::new(cwd).join(".pi/skills"), "project"));
        places.push((Path::new(cwd).join(".agents/skills"), "project"));
    }
    let mut found: Vec<Value> = Vec::new();
    for (dir, scope) in places {
        for path in directories(&dir) {
            let file = path.join("SKILL.md");
            let id = path_text(&path);
            if !file.exists() || found.iter().any(|skill| skill["id"] == id.as_str()) {
                continue;
            }
            let fields = frontmatter(&file);
            let text = |key: &str| fields.get(key).and_then(Value::as_str).unwrap_or_default();
            let name = Some(text("name").to_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| base_name(&id));
            found.push(json!({
                "id": id,
                "name": name,
                "description": clip(text("description"), 240),
                "scope": scope,
                "enabled": !excluded(&settings["skills"], &id),
            }));
        }
    }
    found.sort_by_key(|skill| skill["name"].as_str().unwrap_or_default().to_lowercase());
    Ok(found)
}

fn toggle(config: &Config, key: &str, id: &str, enabled: bool) -> Result<(), String> {
    let file = settings_file(config);
    let mut settings = read_json(&file)?;
    let list = toggled(&settings[key], id, enabled);
    if let Value::Object(fields) = &mut settings {
        fields.insert(key.to_owned(), list);
    }
    write_json(&file, &settings, false)
}

pub(crate) fn set_skill(config: &Config, id: &str, enabled: bool) -> Result<(), String> {
    toggle(config, "skills", id, enabled)
}

pub(crate) fn set_extension(config: &Config, id: &str, enabled: bool) -> Result<(), String> {
    toggle(config, "extensions", id, enabled)
}

fn loads(path: &Path) -> bool {
    let name = path.to_string_lossy();
    name.ends_with(".ts")
        || name.ends_with(".js")
        || (path.is_dir() && !directories(path).is_empty())
}

pub(crate) fn list_extensions(config: &Config, cwd: Option<&str>) -> Result<Vec<Value>, String> {
    let settings = read_json(&settings_file(config))?;
    let off = &settings["extensions"];
    let mut list: Vec<Value> = BUILTIN_EXTENSIONS
        .iter()
        .map(|(id, name, description)| {
            json!({
                "id": id,
                "name": name,
                "description": description,
                "scope": "built in",
                "enabled": !excluded(off, id),
            })
        })
        .collect();
    let mut places = vec![(config.agent_dir.join("extensions"), "user")];
    if let Some(cwd) = cwd {
        places.push((Path::new(cwd).join(".pi/extensions"), "project"));
    }
    for (dir, scope) in places {
        for path in directories(&dir).into_iter().filter(|path| loads(path)) {
            let id = path_text(&path);
            let file = base_name(&id);
            let name = file
                .strip_suffix(".ts")
                .or_else(|| file.strip_suffix(".js"))
                .unwrap_or(&file);
            list.push(json!({
                "id": id,
                "name": name,
                "description": "",
                "scope": scope,
                "enabled": !excluded(off, &id),
            }));
        }
    }
    for source in as_list(&settings["packages"]) {
        let id = source.as_str().or_else(|| source["source"].as_str());
        list.push(json!({
            "id": id,
            "name": id,
            "description": "Installed package",
            "scope": "package",
            "enabled": true,
            "fixed": true,
        }));
    }
    Ok(list)
}

pub(crate) fn list_providers(config: &Config, runtime: &Runtime) -> Result<Value, String> {
    let auth = read_json(&auth_file(config))?;
    let custom = read_json(&models_file(config))?;
    let known: Vec<Value> = PROVIDERS
        .iter()
        .map(|&(id, name, env, oauth, keyless, icon)| {
            let stored = auth[id]["type"].as_str();
            let from_env = env.is_some_and(|env| runtime.var(env).is_some_and(|v| !v.is_empty()));
            let via = match stored {
                Some("oauth") => json!("Signed in"),
                Some(_) => json!("API key"),
                None if from_env => json!(env),
                None => Value::Null,
            };
            json!({
                "id": id,
                "name": name,
                "icon": icon,
                "env": env,
                "oauth": oauth,
                "keyless": keyless,
                "connected": stored.is_some() || from_env,
                "via": via,
                "removable": stored.is_some(),
            })
        })
        .collect();
    let endpoints: Vec<Value> = custom["providers"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(id, provider)| {
            let models: Vec<&str> = as_list(&provider["models"])
                .iter()
                .filter_map(|model| model["id"].as_str())
                .collect();
            json!({
                "id": id,
                "url": provider["baseUrl"].as_str().unwrap_or_default(),
                "models": models.join(", "),
            })
        })
        .collect();
    Ok(json!({ "known": known, "endpoints": endpoints }))
}

fn edit_private(file: &Path, edit: impl FnOnce(&mut Map<String, Value>)) -> Result<(), String> {
    let mut value = read_json(file)?;
    if let Value::Object(fields) = &mut value {
        edit(fields);
    }
    write_json(file, &value, true)
}

pub(crate) fn save_key(config: &Config, provider: &str, key: &str) -> Result<(), String> {
    edit_private(&auth_file(config), |auth| {
        auth.insert(
            provider.to_owned(),
            json!({ "type": "api_key", "key": key }),
        );
    })
}

pub(crate) fn forget_key(config: &Config, provider: &str) -> Result<(), String> {
    edit_private(&auth_file(config), |auth| {
        auth.remove(provider);
    })
}

pub(crate) fn save_endpoint(config: &Config, endpoint: &Value) -> Result<(), String> {
    let id = endpoint["id"].as_str().ok_or("An endpoint needs a name.")?;
    let key = endpoint["key"].as_str().filter(|key| !key.is_empty());
    let models: Vec<Value> = as_list(&endpoint["models"])
        .iter()
        .map(|model| json!({ "id": model, "name": model }))
        .collect();
    edit_private(&models_file(config), |file| {
        let providers = file.entry("providers").or_insert_with(|| json!({}));
        providers[id] = json!({
            "baseUrl": endpoint["url"],
            "api": "openai-completions",
            "apiKey": key.unwrap_or("none"),
            "models": models,
        });
    })
}

pub(crate) fn remove_endpoint(config: &Config, id: &str) -> Result<(), String> {
    edit_private(&models_file(config), |file| {
        if let Some(Value::Object(providers)) = file.get_mut("providers") {
            providers.remove(id);
        }
    })
}

/// What decides which models exist: the credentials held and the custom
/// endpoints, to notice `/login` in a terminal and edits by hand. Not the
/// settings, which pi writes itself as a thread changes model, and not the
/// credentials' contents, as pi refreshes tokens on its own.
pub(crate) fn config_stamp(config: &Config) -> String {
    let models = fs::metadata(models_file(config))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_nanos());
    // Mid-write it reads as nothing; the next look will see it.
    let auth = read_json(&auth_file(config)).unwrap_or_default();
    let mut held: Vec<&str> = auth
        .as_object()
        .into_iter()
        .flatten()
        .map(|(key, _)| key.as_str())
        .collect();
    held.sort_unstable();
    format!("{}/{models}", held.join(","))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn config(root: &Path) -> Config {
        Config {
            agent_dir: root.join("agent"),
            data_dir: root.join("data"),
            home: root.join("home"),
            pi: None,
            login_shell: false,
            env: Vec::new(),
        }
    }

    #[test]
    fn a_key_lands_in_auth_json_private_and_comes_back_out() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        save_key(&config, "groq", "gsk-test").unwrap();
        let file = config.agent_dir.join("auth.json");
        let auth = read_json(&file).unwrap();
        assert_eq!(
            auth["groq"],
            json!({ "type": "api_key", "key": "gsk-test" })
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let runtime = Runtime::new(&config);
        let listed = list_providers(&config, &runtime).unwrap();
        let groq = as_list(&listed["known"])
            .iter()
            .find(|p| p["id"] == "groq")
            .unwrap();
        assert_eq!(groq["connected"], true);
        assert_eq!(groq["via"], "API key");
        assert!(
            !listed.to_string().contains("gsk-test"),
            "a key is never published"
        );
        forget_key(&config, "groq").unwrap();
        assert!(read_json(&file).unwrap().get("groq").is_none());
    }

    #[test]
    fn an_endpoint_with_an_empty_model_table_still_saves() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let endpoint = json!({ "id": "local", "url": "http://x/v1", "key": "", "models": {} });
        save_endpoint(&config, &endpoint).unwrap();
        let saved = read_json(&config.agent_dir.join("models.json")).unwrap();
        assert_eq!(saved["providers"]["local"]["apiKey"], "none");
        let runtime = Runtime::new(&config);
        let listed = list_providers(&config, &runtime).unwrap();
        assert_eq!(listed["endpoints"][0]["url"], "http://x/v1");
        remove_endpoint(&config, "local").unwrap();
        let saved = read_json(&config.agent_dir.join("models.json")).unwrap();
        assert!(saved["providers"].get("local").is_none());
    }

    #[test]
    fn skills_list_with_their_frontmatter_and_turn_off_in_pi_settings() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let skill = config.agent_dir.join("skills/tidy-up");
        fs::create_dir_all(&skill).unwrap();
        let text = "---\nname: \"tidy-up\"\ndescription: Tidies things: up.\n---\nTidy.\n";
        fs::write(skill.join("SKILL.md"), text).unwrap();
        let list = list_skills(&config, None).unwrap();
        assert_eq!(list[0]["name"], "tidy-up");
        assert_eq!(list[0]["description"], "Tidies things: up.");
        assert_eq!(list[0]["enabled"], true);
        let id = list[0]["id"].as_str().unwrap();
        set_skill(&config, id, false).unwrap();
        assert_eq!(list_skills(&config, None).unwrap()[0]["enabled"], false);
        set_skill(&config, id, true).unwrap();
        assert_eq!(list_skills(&config, None).unwrap()[0]["enabled"], true);
    }

    #[test]
    fn extensions_are_the_built_in_ones_files_and_packages() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let folder = config.agent_dir.join("extensions");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("ask.ts"), "").unwrap();
        fs::write(folder.join("notes.txt"), "").unwrap();
        let settings =
            json!({ "packages": ["npm:x", { "source": "git:y" }], "extensions": ["-builtin:mcp"] });
        write_json(&settings_file(&config), &settings, false).unwrap();
        let list = list_extensions(&config, None).unwrap();
        let named = |name: &str| list.iter().find(|item| item["name"] == name);
        assert_eq!(named("MCP").unwrap()["enabled"], false);
        assert_eq!(named("ask").unwrap()["scope"], "user");
        assert!(named("notes").is_none());
        assert_eq!(named("git:y").unwrap()["fixed"], true);
    }

    #[test]
    fn a_broken_file_is_named_not_swallowed() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        fs::create_dir_all(&config.agent_dir).unwrap();
        fs::write(settings_file(&config), "{").unwrap();
        let problem = list_skills(&config, None).unwrap_err();
        assert!(
            problem.starts_with("settings.json cannot be read"),
            "{problem}"
        );
    }

    #[test]
    fn the_stamp_follows_credentials_held_not_their_contents() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let before = config_stamp(&config);
        save_key(&config, "groq", "one").unwrap();
        let held = config_stamp(&config);
        assert_ne!(before, held);
        save_key(&config, "groq", "two").unwrap();
        assert_eq!(held, config_stamp(&config));
    }
}
