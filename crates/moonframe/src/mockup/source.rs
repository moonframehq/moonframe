//! The Luau a mockup file is wrapped in: the file's own lines unchanged
//! inside a function, so its line numbers stay its own, then a mod that
//! draws its variants, for a check, a headless render, or the live tab.

/// Starts line 1, ahead of the file's first line: the file becomes the body
/// of a function whose result is the spec.
const HEAD: &str = "local __spec = (function() ";

/// Closes the function, checks the spec, and turns a body into elements.
const TAIL: &str = r#"
end)()
if type(__spec) ~= "table" then
	error("a mockup file returns a table: { ask, width?, height?, variants = { { name, note?, body } } }", 0)
end
if type(__spec.variants) ~= "table" or #__spec.variants == 0 then
	error("a mockup lists its ideas in `variants`, one table each", 0)
end
for index, variant in __spec.variants do
	if type(variant) ~= "table" or type(variant.name) ~= "string" then
		error(`variant {index} needs a \`name\``, 0)
	end
	if variant.body == nil then
		error(`variant {index} ("{variant.name}") needs a \`body\`: an element, a list, or function(app)`, 0)
	end
end
local function __width(variant)
	return variant.width or __spec.width or 480
end
-- One element, a list of them, or what function(app) returns, as a list.
local function __parts(variant, app)
	local body = variant.body
	if type(body) == "function" then
		body = body(app)
	end
	if type(body) == "table" and body.type == nil then
		return body
	end
	return { body }
end
"#;

/// The file's lines inside [`HEAD`] and [`TAIL`]; `use` and `ui` still work.
/// `export type`, allowed only at a file's top, becomes a plain `type`, which
/// a function may declare, padded so columns stay where they were.
fn wrapped(file: &str) -> String {
    let file: String = file
        .split_inclusive('\n')
        .map(|line| {
            let indent = line.len() - line.trim_start().len();
            match line[indent..].strip_prefix("export type") {
                Some(rest) if rest.starts_with(char::is_whitespace) => {
                    format!("{}       type{rest}", &line[..indent])
                },
                _ => line.to_owned(),
            }
        })
        .collect();
    format!("{HEAD}{file}{TAIL}")
}

/// The line of the live mod that [`closing`] flips.
const DONE_FALSE: &str = "\nlocal DONE = false\n";

/// The live mod `text` with its tab closing: the last `DONE` line, the
/// mod's own, as the file above it may hold one too. `None` when `text` has
/// none, as no mod [`live`] wrote.
pub(super) fn closing(text: &str) -> Option<String> {
    let at = text.rfind(DONE_FALSE)?;
    let mut out = text.to_owned();
    out.replace_range(at..at + DONE_FALSE.len(), "\nlocal DONE = true\n");
    Some(out)
}

/// A Luau string literal of `text`.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// An entry that draws the variant at `index` (from 1) alone, padded, in
/// `appearance` (`light` or `dark`); its state says what the file holds:
/// `{ ask, count, variants = { { name, note, width, height } } }`. Index 0
/// draws every variant one under another, for a check.
pub(super) fn entry(file: &str, index: usize, appearance: &str) -> String {
    let appearance = quoted(appearance);
    format!(
        r#"{wrapped}
local __index = {index}
return mod {{
	init = function()
		local variants = {{}}
		for _, variant in __spec.variants do
			table.insert(variants, {{
				name = variant.name,
				note = variant.note,
				width = __width(variant),
				height = variant.height or __spec.height,
			}})
		end
		local state = {{ ask = __spec.ask, count = #__spec.variants, variants = variants }}
		return state, {{ {{ to = "app:appearance", msg = {appearance} }} }}
	end,
	view = function(state, app)
		local shown = {{}}
		for index, variant in __spec.variants do
			if __index == 0 or __index == index then
				table.insert(shown, ui.column {{
					width = __width(variant),
					height = variant.height or __spec.height,
					hug = true,
					table.unpack(__parts(variant, app)),
				}})
			end
		end
		return ui.column {{ style = {{ padding = 16 }}, gap = "large", table.unpack(shown) }}
	end,
}}
"#,
        wrapped = wrapped(file),
    )
}

/// The live mod: the variants in a tab of the user's window, through the
/// `mockups` component. `round` changes with the file, so the tab comes up
/// again; `done` closes it, before the folder goes.
pub(super) fn live(file: &str, path: &str, round: &str, done: bool) -> String {
    let (path, round) = (quoted(path), quoted(round));
    format!(
        r#"-- Written by `moonframe mockup open {path}`: edit that file and run it
-- again, never this one. `moonframe mockup close` takes it away.
{wrapped}
local mockups = use "mockups"
local ROUND = {round}
local DONE = {done}

return mod {{
	title = "Mockups",
	tab = {{ title = "Mockups", icon = "sparkles" }},

	init = function(previous, props)
		local state = previous or {{}}
		-- Only the mod in its own place opens and closes the tab; the tab
		-- itself is an instance, which gets props.
		if props ~= nil then
			return state
		elseif DONE then
			return state, {{ {{ to = "shell", msg = {{ close_tab = "mod:mockup" }} }} }}
		elseif state.round ~= ROUND then
			return {{ round = ROUND }}, {{ {{ to = "shell", msg = {{ open_tab = "mod:mockup" }} }} }}
		end
		return state
	end,

	view = function(state, app)
		local variants = {{}}
		for _, variant in __spec.variants do
			table.insert(variants, {{
				name = variant.name,
				note = variant.note,
				body = ui.column {{
					width = variant.width,
					height = variant.height or __spec.height,
					table.unpack(__parts(variant, app)),
				}},
			}})
		end
		return mockups {{ ask = __spec.ask, file = {path}, width = __spec.width or 480, variants = variants }}
	end,
}}
"#,
        wrapped = wrapped(file),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_keeps_its_line_numbers() {
        let file = "-- line 1\nlocal x = 2 -- line 2\nreturn {}\n";
        let source = entry(file, 1, "dark");
        let lines: Vec<&str> = source.lines().collect();
        assert!(lines[0].starts_with(HEAD) && lines[0].ends_with("-- line 1"));
        assert_eq!(lines[1], "local x = 2 -- line 2");
        assert_eq!(lines[2], "return {}");
    }

    #[test]
    fn quoted_strings_survive_quotes_and_newlines() {
        assert_eq!(quoted("a \"b\"\\\n"), r#""a \"b\"\\\n""#);
        assert!(
            live("return {}", "/tmp/x \"y\".luau", "1", false).contains(r#""/tmp/x \"y\".luau""#)
        );
    }

    #[test]
    fn closing_flips_the_mods_own_flag() -> Result<(), String> {
        let file = "local DONE = false\nreturn { variants = {} }\n";
        let open = live(file, "f", "r", false);
        let closed = closing(&open).ok_or("no flag")?;
        assert_eq!(closed, live(file, "f", "r", true));
        assert!(closing("return {}").is_none());
        Ok(())
    }

    #[test]
    fn export_type_works_inside_the_wrapper() {
        let file =
            "export type Row = { name: string }\n  export type T = number\nlocal exported = 1\n";
        let source = wrapped(file);
        let lines: Vec<&str> = source.lines().collect();
        assert!(lines[0].ends_with("       type Row = { name: string }"));
        assert_eq!(lines[1], "         type T = number");
        assert_eq!(lines[2], "local exported = 1");
    }

    #[test]
    fn done_closes_the_tab() {
        assert!(live("return {}", "f", "r", true).contains("local DONE = true"));
        assert!(live("return {}", "f", "r", false).contains("local DONE = false"));
    }
}
