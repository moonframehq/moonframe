use {
    crate::{
        Action, Border, Color, Command, Corners, Data, Direction, Node, Paint, Part, Rules, SHELL,
        Shade, Shadow, ShadowLayer, ShadowPreset, Sides, Token, Workspace,
    },
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

// keep-sorted start
mod check_data;
mod checks;
mod components;
mod dictation;
mod elements;
mod entries;
mod entry_scopes;
mod font_size;
mod gestures;
mod history;
mod lightbox;
mod maps;
mod mods;
mod motion;
mod orbit;
mod overlays;
mod packages;
mod paint;
mod pointer;
mod revision_timeline;
mod scrollbar;
mod shell;
mod state;
mod styles;
mod switchyard;
mod task_manager;
mod topics;
mod video;
mod view_diagnostics;
mod views;
mod windows;
mod words;
// keep-sorted end

const SHELL_SOURCE: &str = r#"
return mod {
  init = function(previous) return previous or { left = true } end,
  update = function(state, msg)
    if msg == "toggle" then state.left = not state.left end
  end,
  view = function(state, app)
    return ui.column { grow = true,
      ui.hsplit {
        ui.pane { size = 200, fixed = true, visible = state.left, ui.region "left" },
        ui.pane { ui.mount "counter" },
      },
    }
  end,
}
"#;

const COUNTER: &str = r#"
return mod {
  region = "left",
  init = function(previous) return previous or { count = 0 } end,
  update = function(state, msg)
    if msg == "add" then
      state.count += 1
      return state, { { to = "shell", msg = "toggle" } }
    end
  end,
  view = function(state)
    return ui.panel { title = "Counter",
      ui.text(`count {state.count}`),
      ui.button { "Add", msg = "add" },
    }
  end,
}
"#;

fn write(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

fn app(mods: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "main.luau", SHELL_SOURCE);
    for (name, source) in mods {
        write(dir.path(), &format!("mods/{name}/main.luau"), source);
    }
    dir
}

/// The first child of mod `name` marked broken, as text.
fn broken(workspace: &Workspace, name: &str) -> String {
    let slot = workspace.slot(name).unwrap();
    assert_eq!(slot.error(), None, "the view as a whole still draws");
    slot.broken()
        .first()
        .map(ToString::to_string)
        .unwrap_or_default()
}

fn panel_text(workspace: &Workspace, name: &str) -> String {
    let Some(Node::Panel(panel)) = workspace.slot(name).unwrap().view() else {
        panic!("{name} drew no panel");
    };
    let Node::Text(text) = &panel.children[0] else {
        panic!("no text");
    };
    text.text.clone()
}

const STYLE: &str = r##"
return style {
  radius = 6,
  font_size = 13,
  mono = { family = "Fira Code", size = 12, weight = 500, line_height = 1.2,
           fallbacks = { "Symbols Nerd Font Mono" }, ligatures = false },
  colors = {
    accent = { light = "#4f46e5", dark = "#818cf8" },
    paper = { light = "#fff", dark = "#1f1f23" },
  },
  rules = {
    panel = { radius = 10, background = "paper", shadow = "small", padding = "large" },
    [".loud"] = { color = "danger", weight = "bold", padding = 3 },
    ["item:selected"] = { background = "accent/20" },
    ["item:hover"] = { background = "list-hover" },
  },
}
"##;

const STYLED: &str = r#"
return mod {
  view = function()
    return ui.panel { class = "loud", style = { radius = 2 },
      ui.item { "a", selected = true, class = { "loud" } },
    }
  end,
}
"#;

const BADGE_COMPONENT: &str = r#"
return component {
  title = "Pill",
  category = "Data",
  description = "A labelled value.",
  example = { label = "CPU", value = 12 },
  build = function(props)
    return ui.row {
      ui.text { props.label },
      ui.badge { tostring(props.value) },
    }
  end,
}
"#;

/// The view of a mod made from `source`, or its problem.
fn view_of(source: &str) -> Result<Node, String> {
    let dir = app(&[("probe", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("probe").unwrap();
    match slot.error() {
        Some(error) => Err(error.to_string()),
        None => Ok(slot.view().unwrap().clone()),
    }
}

/// Keeps saved state in memory, and counts the saves.
#[derive(Clone, Default)]
struct Memory(std::rc::Rc<std::cell::RefCell<(std::collections::BTreeMap<String, Data>, usize)>>);

impl crate::Persist for Memory {
    fn state(&self, key: &str) -> Option<Data> {
        self.0.borrow().0.get(key).cloned()
    }

    fn save_state(&self, key: &str, value: &Data) {
        let mut inner = self.0.borrow_mut();
        inner.0.insert(key.to_owned(), value.clone());
        inner.1 += 1;
    }

    fn topics(&self) -> std::collections::BTreeMap<String, Data> {
        self.0
            .borrow()
            .0
            .iter()
            .filter_map(|(key, value)| {
                Some((key.strip_prefix("topic:")?.to_owned(), value.clone()))
            })
            .collect()
    }

    fn save_topic(&self, name: &str, value: &Data) {
        self.0
            .borrow_mut()
            .0
            .insert(format!("topic:{name}"), value.clone());
    }
}

/// The text an instance's view draws.
fn instance_text(workspace: &Workspace, key: &str) -> String {
    match workspace.slot(key).and_then(crate::Slot::view) {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("{key}: {other:?}"),
    }
}
