# Loose values: the limits, one line of JSON, and reading a field that may
# come as a number, a string, a boolean or null.
import json
import re

LIMIT = 2000     # newest messages read from an archive
ROWS = 400       # rows published for one list
REFRESH = 60     # seconds between archive reads


def encode(value):
    """One line of JSON, keys sorted."""
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


# ---- Loose values -----------------------------------------------------------
#
# The messages and the saved view are JSON, so a field may come as a number,
# a string, a boolean or null; these read one the way the views always have.

def truthy(value):
    """null, false, 0, "" and "0" are false; any list or map is true."""
    if isinstance(value, (list, dict)):
        return True
    return value not in (None, False, 0, "", "0")


def text(value):
    """A value as text: null as "", true and false as "1" and "0"."""
    if value is None:
        return ""
    if isinstance(value, bool):
        return "1" if value else "0"
    if isinstance(value, float):
        if value.is_integer() and abs(value) < 1e15:
            return str(int(value))
        return "%.15g" % value
    return str(value)


_NUMBER = re.compile(r"\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)")


def number(value):
    """A value as a number, its leading digits for text, 0 when it has none."""
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, (int, float)):
        return value
    if isinstance(value, str):
        found = _NUMBER.match(value)
        if not found:
            return 0
        n = float(found.group(1))
        return int(n) if n.is_integer() else n
    return 0


def integer(value):
    n = number(value)
    try:
        return int(n)
    except (OverflowError, ValueError):
        return 0


def has_text(value):
    return value is not None and re.search(r"\S", text(value)) is not None

