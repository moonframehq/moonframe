-- Source code with line numbers and syntax colours, read-only. A small
-- highlighter in Luau, by the file's extension: keywords, types (a
-- capitalised name), strings, numbers and comments, with block comments
-- carried from line to line. Long files show `limit` lines and a button
-- for more, so a view stays quick to build and lay out.

type Props = {
	lines: { string }?,
	path: string?,
	limit: number?,
	more: any,
	to: string?,
	total: number?,
}

type Lang = {
	keywords: { [string]: boolean },
	line: string?,
	block: { string }?,
	quotes: string,
}

local function set(words: string): { [string]: boolean }
	local found = {}
	for word in string.gmatch(words, "%S+") do
		found[word] = true
	end
	return found
end

local C_LIKE = { "/*", "*/" }
local LANG_NAMES: { [Lang]: string } = {}
local LANGS: { [string]: Lang } = {
	go = {
		keywords = set [[break case chan const continue default defer else fallthrough for func go goto if import
			interface map package range return select struct switch type var nil true false iota
			string bool int int8 int16 int32 int64 uint uint8 uint16 uint32 uint64 uintptr byte rune
			float32 float64 error any complex64 complex128]],
		line = "//", block = C_LIKE, quotes = "\"'`",
	},
	rs = {
		keywords = set [[as async await break const continue crate dyn else enum extern false fn for if impl in let
			loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where
			while bool u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize f32 f64 str char String Vec Option
			Result Some None Ok Err Box]],
		line = "//", block = C_LIKE, quotes = "\"",
	},
	swift = {
		keywords = set [[associatedtype class deinit enum extension fileprivate func import init inout internal let
			open operator private protocol public rethrows static struct subscript typealias var break case
			continue default defer do else fallthrough for guard if in repeat return switch where while as
			catch false is nil self Self super throw throws true try async await some any final override
			mutating weak lazy]],
		line = "//", block = C_LIKE, quotes = "\"",
	},
	js = {
		keywords = set [[break case catch class const continue debugger default delete do else export extends
			false finally for function if import in instanceof let new null return super switch this throw true
			try typeof var void while with yield async await of undefined from as type interface enum
			implements private public readonly]],
		line = "//", block = C_LIKE, quotes = "\"'`",
	},
	lua = {
		keywords = set [[and break do else elseif end false for function if in local nil not or repeat return then
			true until while continue type export]],
		line = "--", quotes = "\"'`",
	},
	py = {
		keywords = set [[False None True and as assert async await break class continue def del elif else except
			finally for from global if import in is lambda nonlocal not or pass raise return try while with
			yield self]],
		line = "#", quotes = "\"'",
	},
	sh = {
		keywords = set [[if then else elif fi case esac for while until do done in function return local export
			set unset readonly shift exit echo printf]],
		line = "#", quotes = "\"'",
	},
	toml = { keywords = set "true false", line = "#", quotes = "\"'" },
	c = {
		keywords = set [[auto break case char const continue default do double else enum extern float for goto if
			int long register return short signed sizeof static struct switch typedef union unsigned void
			volatile while class namespace public private protected template typename virtual new delete true
			false nullptr bool include define]],
		line = "//", block = C_LIKE, quotes = "\"'",
	},
	kt = {
		keywords = set [[as break class continue do else false for fun if in interface is null object package return
			super this throw true try typealias val var when while import private public internal override
			data sealed open companion]],
		line = "//", block = C_LIKE, quotes = "\"'",
	},
}
for name, lang in LANGS do
	LANG_NAMES[lang] = name
end

local ALIASES = {
	ts = "js", tsx = "js", jsx = "js", mjs = "js", cjs = "js", luau = "lua", bash = "sh", zsh = "sh",
	yml = "toml", yaml = "toml", h = "c", cpp = "c", hpp = "c", cc = "c", m = "c", java = "kt", kts = "kt",
	gradle = "kt", rb = "py",
}

local function lang_of(path: string?): Lang?
	if not path then
		return nil
	end
	local extension = string.match(path, "%.([%w]+)$")
	if not extension then
		return if string.match(path, "[Mm]akefile$") or string.match(path, "justfile$") then LANGS.sh else nil
	end
	extension = string.lower(extension)
	return LANGS[ALIASES[extension] or extension]
end

-- Lines already coloured, by language, whether a block comment was open,
-- and text: a file's lines come back on every redraw, and most are
-- unchanged. Kept for this mod's Luau state, and dropped when it grows.
local seen: { [string]: any } = {}
local seen_count = 0
local SEEN_MOST = 20000

-- A line as runs of { text, class }, and whether a block comment is still
-- open at its end.
local function runs(line: string, lang: Lang, in_block: boolean): ({ { string } }, boolean)
	local out = {}
	local plain = ""
	local function flush()
		if plain ~= "" then
			table.insert(out, { plain, "" })
			plain = ""
		end
	end
	local function push(text: string, class: string)
		flush()
		table.insert(out, { text, class })
	end
	local i, n = 1, #line
	if in_block and lang.block then
		local close = string.find(line, lang.block[2], 1, true)
		if not close then
			return { { line, "code-comment" } }, true
		end
		push(string.sub(line, 1, close + #lang.block[2] - 1), "code-comment")
		i = close + #lang.block[2]
	end
	while i <= n do
		local c = string.sub(line, i, i)
		if lang.line and string.sub(line, i, i + #lang.line - 1) == lang.line then
			push(string.sub(line, i), "code-comment")
			return out, false
		elseif lang.block and string.sub(line, i, i + 1) == lang.block[1] then
			local close = string.find(line, lang.block[2], i + 2, true)
			if not close then
				push(string.sub(line, i), "code-comment")
				return out, true
			end
			push(string.sub(line, i, close + 1), "code-comment")
			i = close + 2
		elseif string.find(lang.quotes, c, 1, true) then
			local j = i + 1
			while j <= n do
				local d = string.sub(line, j, j)
				if d == "\\" then
					j += 2
				elseif d == c then
					break
				else
					j += 1
				end
			end
			push(string.sub(line, i, j), "code-string")
			i = j + 1
		elseif string.match(c, "%d") then
			local word = string.match(line, "^[%w_%.]+", i) or c
			push(word, "code-number")
			i += #word
		elseif string.match(c, "[%a_]") then
			local word = string.match(line, "^[%w_]+", i) or c
			if lang.keywords[word] then
				push(word, "code-keyword")
			elseif string.match(word, "^%u") then
				push(word, "code-type")
			elseif string.sub(line, i + #word, i + #word) == "(" then
				push(word, "code-function")
			else
				plain ..= word
			end
			i += #word
		else
			plain ..= c
			i += 1
		end
	end
	flush()
	return out, false
end

return component {
	title = "Code",
	category = "Projects",
	icon = "code",
	description = "Source code with line numbers and syntax colours by the file's extension; shows `limit` lines, then a Show more button.",
	when = "An editor pane showing a file's text: what `app.projects.file.lines` holds. For a short snippet without colours use `code-view`; for changes use `diff-view`.",
	props = {
		["lines"] = "the file's lines",
		["path"] = "the file's path: its extension picks the colours",
		["limit"] = "how many lines to draw (default 400)",
		["more"] = "message the Show more button sends; no button without it",
		["to"] = "the mod to send it to",
		["total"] = "how many lines the file has, if more than `lines`",
	},
	styles = {
		[".code"] = { font = "mono", gap = 0, padding = { 6, 0 } },
		[".code-row"] = { gap = 0, padding = { 0, 16, 0, 0 } },
		[".code-number"] = { color = "muted-foreground", opacity = 0.7 },
		[".code-gutter"] = { padding = { 0, 18, 0, 12 } },
		[".code-keyword"] = { color = "code-keyword" },
		[".code-type"] = { color = "code-type" },
		[".code-function"] = { color = "code-function" },
		[".code-string"] = { color = "code-string" },
		[".code-number-literal"] = { color = "code-number" },
		[".code-comment"] = { color = "code-comment" },
		[".code-more"] = { padding = { 8, 16 } },
	},
	example = {
		path = "app.go",
		lines = {
			"// Application controls the event loop.",
			"type Application struct {",
			"\tmu      sync.Mutex",
			"\tname    string",
			"\tready   bool",
			"}",
			"",
			"func (a *Application) Run() error {",
			"\treturn nil // 42",
			"}",
		},
	},

	build = function(props: Props)
		local lines = props.lines or {}
		local limit = math.min(#lines, props.limit or 400)
		local lang = lang_of(props.path)
		local width = 8 * #tostring(props.total or #lines) + 2
		local rows = {}
		local in_block = false
		for number = 1, limit do
			local line = lines[number]
			local cells = {}
			if lang and line ~= "" then
				local key = `{LANG_NAMES[lang]}{if in_block then "+" else "-"}{line}`
				local hit = seen[key]
				local found
				if hit then
					found, in_block = hit[1], hit[2]
				else
					found, in_block = runs(line, lang, in_block)
					if seen_count >= SEEN_MOST then
						seen, seen_count = {}, 0
					end
					seen[key] = { found, in_block }
					seen_count += 1
				end
				for _, run in found do
					local class = if run[2] == "code-number" then "code-number-literal" else run[2]
					table.insert(cells, if class == "" then ui.text { run[1] } else ui.text { run[1], class = class })
				end
			else
				table.insert(cells, ui.text { if line == "" then " " else line })
			end
			table.insert(rows, ui.row {
				class = "code-row",
				shrink = false,
				ui.row {
					class = "code-gutter",
					width = width + 30,
					justify = "end",
					shrink = false,
					ui.text { tostring(number), class = "code-number" },
				},
				table.unpack(cells),
			})
		end
		local hidden = (props.total or #lines) - limit
		if hidden > 0 and props.more ~= nil then
			table.insert(rows, ui.row {
				class = "code-more",
				ui.button { `Show {math.min(hidden, 1000)} more lines`, small = true, variant = "ghost", msg = props.more, to = props.to },
				ui.text { `{hidden} not shown`, tone = "muted", size = "small" },
			})
		end
		return ui.column { class = "code", scroll = "both", grow = true, table.unpack(rows) }
	end,
}
