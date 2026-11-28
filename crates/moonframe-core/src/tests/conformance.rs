//! The conformance suite, `conformance/` at the repository root: small apps
//! that Moonframe and MoonSwift both load, play and compare, so a mod that
//! stays in the shared core draws the same on both. `conformance/README.md`
//! is the format. Moonframe is the reference: `CONFORMANCE_BLESS=1` fills
//! each `expect` left `null` with what it draws.

use {
    crate::{Appearance, Data, IconName, SHELL, Workspace},
    mlua::Value as Lua,
    serde_json::{Map, Value as Json},
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
    },
};

/// The most rows of a list a tree carries.
const MAX_ROWS: i64 = 50;

#[test]
fn conformance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases");
    let bless = std::env::var("CONFORMANCE_BLESS").is_ok_and(|value| value == "1");
    let mut cases: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|dir| dir.join("case.json").is_file())
        .collect();
    cases.sort();
    assert!(!cases.is_empty(), "no cases in {}", root.display());
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|dir| {
            let name = dir.file_name().unwrap().to_string_lossy();
            run(dir, bless)
                .err()
                .map(|failure| format!("{name}: {failure}"))
        })
        .collect();
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

/// Every icon name a portable mod may use is one Moonframe bundles.
#[test]
fn shared_icons_are_bundled() {
    let names = include_str!("../../../../conformance/icons.txt");
    let missing: Vec<&str> = names
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| {
            IconName::new(name)
                .and_then(|icon| icon.bundled())
                .is_none()
        })
        .collect();
    assert!(missing.is_empty(), "not bundled: {missing:?}");
}

/// Plays the case in `dir`, and with `bless` writes what it drew into the
/// `expect` trees left `null`.
fn run(dir: &Path, bless: bool) -> Result<(), String> {
    let path = dir.join("case.json");
    let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut case: Json = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    // The case alone: no user folder, nothing saved, no providers.
    let mut workspace = Workspace::open(dir, false);
    workspace.set_appearance(Appearance::Light, BTreeMap::new());
    no_problems(&workspace).map_err(|problems| format!("loading: {problems}"))?;
    let steps = case
        .get_mut("steps")
        .and_then(Json::as_array_mut)
        .ok_or("no `steps`")?;
    let mut blessed = false;
    for (index, step) in steps.iter_mut().enumerate() {
        play(&mut workspace, step, bless, &mut blessed)
            .and_then(|()| no_problems(&workspace))
            .map_err(|failure| format!("step {}: {failure}", index + 1))?;
    }
    if blessed {
        let text = serde_json::to_string_pretty(&sorted(&case)).map_err(|e| e.to_string())?;
        fs::write(&path, text + "\n").map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn play(
    workspace: &mut Workspace,
    step: &mut Json,
    bless: bool,
    blessed: &mut bool,
) -> Result<(), String> {
    let Json::Object(step) = step else {
        return Err(format!("a step is an object, not {step}"));
    };
    if let Some(msg) = step.get("send") {
        let to = step
            .get("to")
            .and_then(Json::as_str)
            .ok_or("`send` without `to`")?;
        // As a click on one of its own controls.
        workspace.send(to, to, data(msg));
    } else if let Some(name) = step.get("publish") {
        let name = name.as_str().ok_or("`publish` names a provider")?;
        workspace.set_host(name, data(step.get("data").unwrap_or(&Json::Null)));
    } else if let Some(expect) = step.get_mut("expect") {
        let Json::Object(expect) = expect else {
            return Err("`expect` is an object of trees by mod".to_owned());
        };
        for (name, want) in expect {
            kept_is_current(workspace, name)?;
            let got = view_tree(workspace, name)?;
            if want.is_null() {
                if !bless {
                    return Err(format!(
                        "`{name}` expects null: run with CONFORMANCE_BLESS=1 to fill it in"
                    ));
                }
                *want = got;
                *blessed = true;
            } else if !same(want, &got) {
                return Err(differ(name, want, &got));
            }
        }
    } else if let Some(want) = step.get("outbox") {
        let got = Json::Array(
            workspace
                .take_outbox()
                .into_iter()
                .map(|(to, from, msg)| {
                    serde_json::json!({ "to": to, "from": from, "msg": json(&msg) })
                })
                .collect(),
        );
        if !same(want, &got) {
            return Err(differ("outbox", want, &got));
        }
    } else if let Some(Json::Object(regions)) = step.get("regions") {
        for (region, want) in regions {
            let got = Json::from(workspace.region(region));
            if !same(want, &got) {
                return Err(differ(&format!("region {region}"), want, &got));
            }
        }
    } else {
        return Err(format!("an unknown step: {}", Json::Object(step.clone())));
    }
    Ok(())
}

/// Every problem, and every note a view drew, as text; none is `Ok`.
fn no_problems(workspace: &Workspace) -> Result<(), String> {
    let problems = workspace
        .problems()
        .into_iter()
        .map(|(name, error)| format!("{name}: {error}"));
    let notes = std::iter::once(SHELL)
        .chain(workspace.mod_names())
        .filter_map(|name| Some((name, workspace.slot(name)?)))
        .flat_map(|(name, slot)| {
            slot.notes()
                .iter()
                .map(move |note| format!("{name}: {note}"))
        });
    let all: Vec<String> = problems.chain(notes).collect();
    if all.is_empty() {
        Ok(())
    } else {
        Err(format!("problems:\n  {}", all.join("\n  ")))
    }
}

fn differ(what: &str, want: &Json, got: &Json) -> String {
    let pretty = |json: &Json| serde_json::to_string_pretty(&sorted(json)).unwrap_or_default();
    format!(
        "{what} differs\n--- expected\n{}\n--- drawn\n{}",
        pretty(want),
        pretty(got)
    )
}

/// Whether the view the workspace keeps for `name`, the one a window shows,
/// is what its view draws now: a send or a publish that changed what it
/// draws without drawing it again leaves the window stale, though the tree
/// `view_tree` builds afresh would still match.
fn kept_is_current(workspace: &Workspace, name: &str) -> Result<(), String> {
    let fresh = workspace
        .fresh_view(name)
        .ok_or_else(|| format!("no mod `{name}` loaded"))?
        .map_err(|error| format!("{name}: {error}"))?;
    let kept = workspace
        .slot(name)
        .and_then(|slot| slot.view())
        .ok_or_else(|| format!("`{name}` has drawn nothing"))?;
    if *kept != fresh {
        return Err(format!(
            "`{name}` shows a stale view: what it draws now changed, but it was not drawn again"
        ));
    }
    Ok(())
}

/// The view of the mod `name` as a tree.
fn view_tree(workspace: &Workspace, name: &str) -> Result<Json, String> {
    let value = workspace
        .raw_view(name)
        .ok_or_else(|| format!("no mod `{name}` loaded"))?
        .map_err(|error| format!("{name}: {error}"))?;
    tree(&value).map_err(|error| format!("{name}: {error}"))
}

/// A value of a view as the README writes it.
fn tree(value: &Lua) -> Result<Json, String> {
    Ok(match value {
        Lua::Nil => Json::Null,
        Lua::Boolean(value) => Json::Bool(*value),
        Lua::Integer(value) => Json::from(*value),
        Lua::Number(value) => number(*value)?,
        Lua::String(value) => Json::String(value.to_string_lossy()),
        Lua::Function(_) => Json::from("<function>"),
        Lua::Table(table) => table_tree(table)?,
        other => return Err(format!("a {} in the view", other.type_name())),
    })
}

/// A list part alone is an array; string keys make an object, with the list
/// part, `false` left out, as `children`. A list whose `row` is a function
/// also gets `rows`.
fn table_tree(table: &mlua::Table) -> Result<Json, String> {
    let len = table.raw_len();
    let mut keys = BTreeMap::new();
    for pair in table.pairs::<Lua, Lua>() {
        let (key, value) = pair.map_err(|error| error.to_string())?;
        match key {
            Lua::String(key) => {
                keys.insert(key.to_string_lossy(), value);
            },
            key if index(&key).is_some_and(|index| (1..=len).contains(&index)) => {},
            key => return Err(format!("a key {key:?} outside the list part")),
        }
    }
    let list = (1..=len)
        .map(|index| {
            table
                .raw_get::<Lua>(index)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if keys.is_empty() {
        return list
            .iter()
            .map(tree)
            .collect::<Result<_, _>>()
            .map(Json::Array);
    }
    let mut object = Map::new();
    for (key, value) in &keys {
        if key != "__line" {
            object.insert(key.clone(), tree(value)?);
        }
    }
    let children = list
        .iter()
        .filter(|child| !matches!(child, Lua::Boolean(false)))
        .map(tree)
        .collect::<Result<Vec<_>, _>>()?;
    if !children.is_empty()
        && object
            .insert("children".to_owned(), Json::Array(children))
            .is_some()
    {
        return Err("a `children` key beside a list part".to_owned());
    }
    if let (Some(Lua::String(kind)), Some(Lua::Function(row))) = (keys.get("type"), keys.get("row"))
        && kind.to_string_lossy() == "list"
    {
        let count = keys.get("count").and_then(index).unwrap_or(0);
        let rows = (1..=count.min(MAX_ROWS as usize))
            .map(|index| {
                row.call::<Lua>(index)
                    .map_err(|error| format!("row({index}): {error}"))
                    .and_then(|value| tree(&value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        object.insert("rows".to_owned(), Json::Array(rows));
    }
    Ok(Json::Object(object))
}

/// A whole number at least 1, as a list index or a count.
fn index(value: &Lua) -> Option<usize> {
    match value {
        Lua::Integer(value) => usize::try_from(*value).ok(),
        Lua::Number(value) if value.fract() == 0.0 && *value >= 0.0 => Some(*value as usize),
        _ => None,
    }
}

/// A whole number written without a fraction, as both hosts compare them.
fn number(value: f64) -> Result<Json, String> {
    if value.fract() == 0.0 && value.abs() < 2f64.powi(53) {
        return Ok(Json::from(value as i64));
    }
    serde_json::Number::from_f64(value)
        .map(Json::Number)
        .ok_or_else(|| format!("the number {value}"))
}

/// A step's message or data as a mod gets it.
fn data(json: &Json) -> Data {
    match json {
        Json::Null => Data::Nil,
        Json::Bool(value) => Data::Bool(*value),
        Json::Number(value) => match value.as_i64() {
            Some(value) => Data::Integer(value),
            None => Data::Number(value.as_f64().unwrap_or_default()),
        },
        Json::String(value) => Data::String(value.clone()),
        Json::Array(values) => Data::List(values.iter().map(data).collect()),
        Json::Object(fields) => Data::Map(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), data(value)))
                .collect(),
        ),
    }
}

/// What a mod sent, as a tree writes it: the empty table is an array.
fn json(data: &Data) -> Json {
    match data {
        Data::Nil => Json::Null,
        Data::Bool(value) => Json::Bool(*value),
        Data::Integer(value) => Json::from(*value),
        Data::Number(value) => number(*value).unwrap_or(Json::Null),
        Data::String(value) => Json::from(value.as_str()),
        Data::List(values) => Json::Array(values.iter().map(json).collect()),
        Data::Map(fields) if fields.is_empty() => Json::Array(Vec::new()),
        Data::Map(fields) => Json::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), json(value)))
                .collect(),
        ),
    }
}

/// Whether two trees are equal, numbers compared as numbers.
fn same(want: &Json, got: &Json) -> bool {
    match (want, got) {
        (Json::Number(want), Json::Number(got)) => want.as_f64() == got.as_f64(),
        (Json::Array(want), Json::Array(got)) => {
            want.len() == got.len() && want.iter().zip(got).all(|(want, got)| same(want, got))
        },
        (Json::Object(want), Json::Object(got)) => {
            want.len() == got.len()
                && want
                    .iter()
                    .all(|(key, want)| got.get(key).is_some_and(|got| same(want, got)))
        },
        _ => want == got,
    }
}

/// `json` with every object's keys in order, however the map keeps them.
fn sorted(json: &Json) -> Json {
    match json {
        Json::Array(values) => Json::Array(values.iter().map(sorted).collect()),
        Json::Object(fields) => {
            let fields: BTreeMap<&String, Json> = fields
                .iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            Json::Object(
                fields
                    .into_iter()
                    .map(|(key, value)| (key.clone(), value))
                    .collect(),
            )
        },
        other => other.clone(),
    }
}
