"""Conservative Luau dependency scanning, without executing app code.

This is a lexer, not a whole-program analyser. Literal imports are followed;
computed imports and indexed provider/UI access need explicit build hints.
Comments and quoted documentation never masquerade as executable code.
"""

from dataclasses import dataclass, field
import re


@dataclass
class Token:
    kind: str
    value: str
    line: int


class Lexer:
    def __init__(self, text):
        self.text = text
        self.at = 0

    def long_string(self):
        match = re.match(r"\[(=*)\[", self.text[self.at:])
        if not match:
            return None
        end = "]" + match[1] + "]"
        start = self.at + len(match[0])
        stop = self.text.find(end, start)
        if stop < 0:
            raise ValueError("unclosed long string/comment")
        self.at = stop + len(end)
        return self.text[start:stop].removeprefix("\n")

    def quoted(self):
        quote = self.text[self.at]
        self.at += 1
        value = ""
        while self.at < len(self.text):
            char = self.text[self.at]
            self.at += 1
            if char == quote:
                return value
            if char == "\\":
                escape = self.text[self.at:]
                number = re.match(r"[0-9]{1,3}", escape)
                hex_byte = re.match(r"x([0-9a-fA-F]{2})", escape)
                unicode = re.match(r"u\{([0-9a-fA-F]+)\}", escape)
                if number:
                    value += chr(int(number[0]))
                    self.at += len(number[0])
                elif hex_byte or unicode:
                    match = hex_byte or unicode
                    value += chr(int(match[1], 16))
                    self.at += len(match[0])
                elif escape.startswith("z"):
                    self.at += 1
                    while self.at < len(self.text) and self.text[self.at].isspace():
                        self.at += 1
                elif escape:
                    value += {"n": "\n", "r": "\r", "t": "\t"}.get(escape[0], escape[0])
                    self.at += 1
                else:
                    raise ValueError("unclosed escape")
            else:
                value += char
        raise ValueError("unclosed string")

    def tokens(self, stop=None):
        result = []
        while self.at < len(self.text):
            char = self.text[self.at]
            line = self.text.count("\n", 0, self.at) + 1
            if char == stop:
                self.at += 1
                return result
            if char.isspace():
                self.at += 1
                continue
            if self.text.startswith("--", self.at):
                self.at += 2
                if self.long_string() is None:
                    end = self.text.find("\n", self.at)
                    self.at = len(self.text) if end < 0 else end
                continue
            if char in "\"'":
                result.append(Token("string", self.quoted(), line))
                continue
            if char == "[":
                value = self.long_string()
                if value is not None:
                    result.append(Token("string", value, line))
                    continue
            if char == "`":
                self.at += 1
                result.append(Token("interpolated", "`", line))
                literal = ""
                while self.at < len(self.text) and self.text[self.at] != "`":
                    if self.text[self.at] == "\\":
                        self.at += 2
                    elif self.text[self.at] == "{":
                        self.at += 1
                        result.extend(self.tokens("}"))
                    else:
                        literal += self.text[self.at]
                        self.at += 1
                if self.at == len(self.text):
                    raise ValueError("unclosed interpolation")
                self.at += 1
                # A computed destination cannot be resolved from its prefix.
                if "provider:" in literal or "provider/" in literal:
                    result.append(Token("dynamic-provider", literal, line))
                continue
            if char == "{" and stop == "}":
                self.at += 1
                result.append(Token("symbol", "{", line))
                result.extend(self.tokens("}"))
                result.append(Token("symbol", "}", line))
                continue
            name = re.match(r"[a-zA-Z_][a-zA-Z_0-9]*", self.text[self.at:])
            if name:
                self.at += len(name[0])
                result.append(Token("name", name[0], line))
            else:
                operator = next((op for op in ("...", "..", "::")
                                 if self.text.startswith(op, self.at)), char)
                result.append(Token("symbol", operator, line))
                self.at += len(operator)
        if stop:
            raise ValueError(f"unclosed {stop}")
        return result


@dataclass
class References:
    components: set = field(default_factory=set)
    providers: set = field(default_factory=set)
    features: set = field(default_factory=set)
    dynamic: list = field(default_factory=list)


def terminal_engine(tokens, at):
    """Recognise an explicit engine in a literal ui.terminal property table.

    Other forms retain the default; their additional engines need hints.
    """
    if at < len(tokens) and tokens[at].value == "(":
        at += 1
    if at >= len(tokens) or tokens[at].value != "{":
        return "alacritty"
    depth = 0
    for i in range(at, len(tokens)):
        token = tokens[i]
        if token.kind == "symbol" and token.value == "{":
            depth += 1
        elif token.kind == "symbol" and token.value == "}":
            depth -= 1
            if depth == 0:
                break
        if depth == 1 and token.kind == "name" and token.value == "engine":
            following = tokens[i + 1:i + 4]
            if len(following) == 3 and following[0].value == "=" and following[1].kind == "string" and following[2].value in (",", ";", "}"):
                if following[1].value in ("alacritty", "ghostty"):
                    return following[1].value
    return "alacritty"


def references(text, provider_names):
    tokens = Lexer(text).tokens()
    refs = References()
    empty = Token("", "", 0)
    for i, token in enumerate(tokens):
        before = tokens[i - 1] if i else empty
        after = tokens[i + 1] if i + 1 < len(tokens) else empty
        next2 = tokens[i + 2] if i + 2 < len(tokens) else empty
        next3 = tokens[i + 3] if i + 3 < len(tokens) else empty
        if token.kind == "name" and token.value == "use" and before.value not in (".", ":"):
            literal = after if after.kind == "string" else next2
            if after.kind == "string" or (after.value == "(" and next2.kind == "string" and next3.value == ")"):
                refs.components.add(literal.value)
            else:
                refs.dynamic.append(("components", token.line))
        if token.kind == "name" and token.value in ("app", "ui"):
            category = "providers" if token.value == "app" else "features"
            name = None
            if after.value == "." and next2.kind == "name":
                name = next2.value
            elif after.value == "[":
                if next2.kind == "string" and next3.value == "]":
                    name = next2.value
                else:
                    refs.dynamic.append((category, token.line))
            elif before.value == "=":
                refs.dynamic.append((category, token.line))
            if category == "providers" and name in provider_names:
                refs.providers.add(name)
            if category == "features" and name in ("terminal", "browser", "markdown"):
                refs.features.add(name)
                if name == "terminal":
                    refs.features.add(terminal_engine(tokens, i + (3 if after.value == "." else 4)))
        if token.kind == "dynamic-provider":
            refs.dynamic.append(("providers", token.line))
        if token.kind == "string":
            for prefix, target in (("provider:", refs.providers), ("provider/", refs.providers),
                                   ("component:", refs.components)):
                if token.value.startswith(prefix):
                    name = token.value[len(prefix):]
                    if re.fullmatch(r"[a-z0-9_-]+", name) and after.value != "..":
                        target.add(name)
                    else:
                        refs.dynamic.append(("components" if prefix == "component:" else "providers", token.line))
            if token.value == "ghostty":
                refs.features.add("ghostty")
            if token.value == "herdr-core":
                refs.features.add("herdr-core")
    return refs
