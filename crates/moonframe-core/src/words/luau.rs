//! `ui.t`, `ui.number`, `ui.date` and `ui.locale`: what a module sees of
//! its app's words.

use {
    super::{
        Words,
        format::{Clock, DateLength, DateStyle, MAX_DECIMALS},
    },
    crate::{
        Error,
        read::{Cx, Fields, NO_NAMES},
    },
    fluent_bundle::{FluentArgs, FluentValue},
    mlua::{Lua, Table, Value},
};

/// The most arguments one message takes.
const MAX_ARGS: usize = 64;

impl Words {
    /// Adds the functions to `ui`, which the prelude made and the sandbox
    /// has not yet frozen.
    pub(crate) fn install(&self, lua: &Lua) -> mlua::Result<()> {
        let ui: Table = lua.globals().raw_get("ui")?;
        let words = self.clone();
        ui.raw_set(
            "t",
            lua.create_function(move |_, (id, args): (String, Option<Table>)| {
                let args = args.map(|args| arguments(&id, &args)).transpose()?;
                Ok(words.text(&id, args.as_ref()))
            })?,
        )?;
        let words = self.clone();
        ui.raw_set(
            "number",
            lua.create_function(move |_, (value, options): (f64, Value)| {
                let decimals = decimals(&options).map_err(mlua::Error::external)?;
                Ok(words.formats().number(value, decimals))
            })?,
        )?;
        let words = self.clone();
        ui.raw_set(
            "date",
            lua.create_function(move |_, (seconds, options): (f64, Value)| {
                let style = date_style(&options).map_err(mlua::Error::external)?;
                words.formats().date(seconds, style).ok_or_else(|| {
                    mlua::Error::external(Error::new(format!(
                        "ui.date: {seconds} is not a time it can write"
                    )))
                })
            })?,
        )?;
        let locale = self.locale();
        ui.raw_set(
            "locale",
            lua.create_function(move |_, ()| Ok(locale.clone()))?,
        )
    }
}

/// A message's `{ count = 3, name = "Ada" }`: numbers stay numbers, so a
/// message can pick its plural by them.
fn arguments(id: &str, table: &Table) -> mlua::Result<FluentArgs<'static>> {
    let mut args = FluentArgs::new();
    for pair in table.pairs::<Value, Value>() {
        let (key, value) = pair?;
        if args.iter().count() >= MAX_ARGS {
            return Err(fail(format!(
                "ui.t(\"{id}\"): more than {MAX_ARGS} arguments"
            )));
        }
        let Value::String(key) = key else {
            return Err(fail(format!(
                "ui.t(\"{id}\"): arguments are named, such as {{ count = 3 }}"
            )));
        };
        let key = key.to_string_lossy();
        let value = match value {
            Value::Integer(number) => FluentValue::from(number),
            Value::Number(number) => FluentValue::from(number),
            Value::String(text) => FluentValue::from(text.to_string_lossy()),
            Value::Boolean(flag) => FluentValue::from(flag.to_string()),
            other => {
                return Err(fail(format!(
                    "ui.t(\"{id}\"): `{key}` is a {}, not a string or number",
                    other.type_name()
                )));
            },
        };
        args.set(key, value);
    }
    Ok(args)
}

fn fail(message: String) -> mlua::Error {
    mlua::Error::external(Error::new(message))
}

/// `ui.number`'s `{ decimals = 2 }`.
fn decimals(options: &Value) -> Result<Option<usize>, Error> {
    if options.is_nil() {
        return Ok(None);
    }
    let cx = Cx::root(&NO_NAMES).key("ui.number");
    let fields = Fields::read(options, &cx, &[&["decimals"]])?;
    let Some(decimals) = fields.get::<i64>("decimals")? else {
        return Ok(None);
    };
    usize::try_from(decimals)
        .ok()
        .filter(|decimals| *decimals <= MAX_DECIMALS)
        .map(Some)
        .ok_or_else(|| {
            cx.key("decimals")
                .fail(format!("expected 0 to {MAX_DECIMALS}"))
        })
}

/// `ui.date`'s `{ style = "short", date = false, time = true }`.
fn date_style(options: &Value) -> Result<DateStyle, Error> {
    let mut style = DateStyle {
        length: DateLength::default(),
        date: true,
        time: None,
    };
    if options.is_nil() {
        return Ok(style);
    }
    let cx = Cx::root(&NO_NAMES).key("ui.date");
    let fields = Fields::read(options, &cx, &[&["style", "date", "time"]])?;
    if let Some(length) = fields.get::<String>("style")? {
        style.length = match length.as_str() {
            "short" => DateLength::Short,
            "medium" => DateLength::Medium,
            "long" => DateLength::Long,
            _ => {
                return Err(cx
                    .key("style")
                    .fail("expected \"short\", \"medium\" or \"long\""));
            },
        };
    }
    style.date = fields.get::<bool>("date")?.unwrap_or(true);
    style.time = match fields.raw("time") {
        None | Some(Value::Nil | Value::Boolean(false)) => None,
        Some(Value::Boolean(true)) => Some(Clock::Minutes),
        Some(Value::String(clock)) if clock == "minutes" => Some(Clock::Minutes),
        Some(Value::String(clock)) if clock == "seconds" => Some(Clock::Seconds),
        Some(_) => {
            return Err(cx
                .key("time")
                .fail("expected true, false, \"minutes\" or \"seconds\""));
        },
    };
    if !style.date && style.time.is_none() {
        return Err(cx.fail("shows nothing: give it `time`, or leave `date` on"));
    }
    Ok(style)
}
