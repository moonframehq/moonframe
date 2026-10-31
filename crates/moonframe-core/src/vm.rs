//! The sandboxed Luau state each module runs in: only the safe standard
//! libraries, a memory limit, an instruction budget per call, and globals
//! frozen once `ui`, `mod`, `style`, `component`, `use`, and `log` are in
//! place.

use {
    crate::component::Library,
    mlua::{
        Lua, LuaOptions, StdLib, Value, Variadic, VmState,
        chunk::{Chunk, ChunkMode},
    },
    std::{cell::Cell, rc::Rc},
};

const PRELUDE: &str = include_str!("prelude.luau");

/// Luau's interrupt checks per call: one per loop iteration and call, roughly.
const INTERRUPTS: u64 = 10_000_000;
const MEMORY: usize = 64 * 1024 * 1024;

/// Globals a module never gets: code loading, environments, and the
/// collector. `print` is replaced by `log`.
const REMOVED: &[&str] = &[
    "collectgarbage",
    "gcinfo",
    "getfenv",
    "loadstring",
    "newproxy",
    "print",
    "require",
    "setfenv",
];

#[derive(Debug, thiserror::Error)]
#[error("ran out of instructions; is there an endless loop?")]
struct OutOfInstructions;

/// Counts interrupt checks; reset before each call so each gets the whole
/// budget.
#[derive(Clone, Debug, Default)]
pub(crate) struct Meter(Rc<Cell<u64>>);

impl Meter {
    pub(crate) fn reset(&self) {
        self.0.set(0);
    }
}

/// A sealed state for the module `name`, ready to load its source, with
/// `library`'s components to `use`.
pub(crate) fn open(name: &str, meter: &Meter, library: &Library) -> mlua::Result<Lua> {
    let libs =
        StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8 | StdLib::BIT | StdLib::BUFFER;
    let lua = Lua::new_with(libs, LuaOptions::new())?;
    let globals = lua.globals();
    for removed in REMOVED {
        globals.raw_set(*removed, Value::Nil)?;
    }
    lua.set_memory_limit(MEMORY)?;
    let count = meter.0.clone();
    lua.set_interrupt(move |_| {
        let used = count.get().saturating_add(1);
        count.set(used);
        if used > INTERRUPTS {
            return Err(mlua::Error::external(OutOfInstructions));
        }
        Ok(VmState::Continue)
    });

    // The prelude keeps this for itself and the global goes: `ui` stamps
    // each element with the line that built it, for the inspector.
    globals.raw_set(CALLER_LINE, caller_line(&lua, name)?)?;
    let (ui, define, style, component): (Value, Value, Value, Value) =
        text(&lua, PRELUDE, "prelude").eval()?;
    globals.raw_set(CALLER_LINE, Value::Nil)?;
    globals.raw_set("ui", ui)?;
    globals.raw_set("mod", define)?;
    globals.raw_set("style", style)?;
    globals.raw_set("component", component)?;
    library.install(&lua)?;
    let prefix = format!("[{name}]");
    let log = lua.create_function(move |_, values: Variadic<Value>| {
        let line = values
            .iter()
            .map(|value| match value {
                Value::String(text) => text.to_string_lossy(),
                Value::Integer(value) => value.to_string(),
                Value::Number(value) => value.to_string(),
                Value::Boolean(value) => value.to_string(),
                Value::Nil => "nil".to_owned(),
                other => other.type_name().to_owned(),
            })
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!("{prefix} {}", printable(&line));
        Ok(())
    })?;
    globals.raw_set("log", log)?;
    meter.reset();
    lua.sandbox(true)?;
    Ok(lua)
}

const CALLER_LINE: &str = "__caller_line";
/// Frames looked through for the module's own code.
const MAX_FRAMES: usize = 64;

/// A function giving the line of the module `name`'s source that is running
/// further up the stack: where a `ui` call was made, or, from inside a
/// component, where the module used the component.
fn caller_line(lua: &Lua, name: &str) -> mlua::Result<mlua::Function> {
    let chunk = format!("={name}");
    lua.create_function(move |lua, ()| {
        for level in 1..MAX_FRAMES {
            let frame = lua.inspect_stack(level, |debug| {
                let ours = debug.source().source.as_deref() == Some(chunk.as_str());
                ours.then(|| debug.current_line()).flatten()
            });
            match frame {
                // Past the bottom of the stack.
                None => return Ok(None),
                Some(Some(line)) => return Ok(i64::try_from(line).ok()),
                Some(None) => {},
            }
        }
        Ok(None)
    })
}

/// Longer log lines are cut.
const MAX_LOG: usize = 4096;

/// A log line safe for a terminal: control characters, which could move the
/// cursor or rewrite earlier output, become `?`, and the line is capped.
pub fn printable(line: &str) -> String {
    line.chars()
        .take(MAX_LOG)
        .map(|char| match char {
            '\t' => char,
            char if char.is_control() => '?',
            char => char,
        })
        .collect()
}

/// A script's source as a chunk named `name` in errors. Always text: Luau
/// bytecode is not verified when loaded, so a file that looks like bytecode
/// could escape the sandbox; in text mode it is a syntax error.
pub(crate) fn text<'a>(lua: &Lua, source: &'a str, name: &str) -> Chunk<'a> {
    lua.load(source)
        .set_mode(ChunkMode::Text)
        .set_name(format!("={name}"))
}
