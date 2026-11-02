import math
import re
import time

from values import number, text, truthy

# ---- formatting, as msgvault-gpui's format.rs --------------------------

WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]
DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]


def local(t):
    return time.localtime(math.floor(number(t)))


def weekday(l):
    # struct_time counts from Monday; the tables from Sunday.
    return (l.tm_wday + 1) % 7


def clock(t):
    l = local(t)
    return "%d:%02d %s" % (l.tm_hour % 12 or 12, l.tm_min, "AM" if l.tm_hour < 12 else "PM")


def day_number(t):
    l = local(t)
    # Local noon of that day, so daylight saving changes never move it.
    return math.floor(time.mktime((l.tm_year, l.tm_mon, l.tm_mday, 12, 0, 0, 0, 0, -1)) / 86400)


def list_date(t, now):
    if not truthy(t):
        return ""
    days = day_number(now) - day_number(t)
    l = local(t)
    n = local(now)
    if days == 0:
        return clock(t)
    if days == 1:
        return "Yesterday"
    if 2 <= days < 7:
        return WEEKDAYS[weekday(l)]
    if l.tm_year == n.tm_year:
        return "%s %d" % (MONTHS[l.tm_mon - 1], l.tm_mday)
    return "%s %d, %d" % (MONTHS[l.tm_mon - 1], l.tm_mday, l.tm_year)


def full_date(t):
    if not truthy(t):
        return ""
    l = local(t)
    return "%s, %s %d, %d at %s" % (DAYS[weekday(l)], MONTHS[l.tm_mon - 1], l.tm_mday,
                                    l.tm_year, clock(t))


def size_text(n):
    units = ["B", "KB", "MB", "GB", "TB"]
    value, unit = number(n), 0
    while value >= 1024 and unit < len(units) - 1:
        value /= 1024
        unit += 1
    if unit == 0:
        return "%s B" % text(n)
    if value >= 100:
        return "%.0f %s" % (value, units[unit])
    return "%.1f %s" % (value, units[unit])


def count(n):
    n = text(n)
    while True:
        grouped = re.sub(r"^(\d+)(\d{3})", r"\1,\2", n)
        if grouped == n:
            return n
        n = grouped


def plural(n, one, many):
    return count(n) + " " + (one if number(n) == 1 else many)


def fnv(value):
    h = 2166136261
    for byte in value.encode("utf-8"):
        h = ((h ^ byte) * 16777619) & 0xFFFFFFFF
    return h


# A label's colour when it has none stored: format::label_color.
PALETTE = ["#4a86e8", "#8e63ce", "#16a766", "#f2a60c", "#e66550", "#2da2bb", "#d5608c",
           "#3dc789", "#7a8cff", "#c28b3c"]


def label_color(name, stored=None):
    if stored is not None and re.match(r"^#[0-9a-fA-F]{6}$", text(stored)):
        return text(stored).lower()
    return PALETTE[fnv(text(name)) % len(PALETTE)]


SYSTEM = {"INBOX", "SENT", "TRASH", "SPAM", "DRAFT", "DRAFTS", "UNREAD", "STARRED",
          "IMPORTANT", "CHAT"}
STATE = {"UNREAD", "STARRED", "INBOX", "SENT", "DRAFT", "IMPORTANT"}


def title_case(word):
    word = text(word).lower()
    return word[:1].title() + word[1:]


def display_label(name):
    name = text(name)
    upper = name.upper()
    category = re.match(r"^CATEGORY_(.+)$", upper)
    if category:
        return title_case(category.group(1))
    if upper in SYSTEM:
        return title_case(upper)
    return name


KINDS = {
    "": "Email", "email": "Email", "imessage": "iMessage", "sms": "SMS", "mms": "MMS",
    "rcs": "RCS", "whatsapp": "WhatsApp", "fbmessenger": "Messenger", "teams": "Teams",
    "slack": "Slack", "discord": "Discord", "beeper": "Beeper", "google_chat": "Google Chat",
    "calendar_event": "Calendar", "meeting_transcript": "Meetings", "matrix": "Matrix",
}


def display_kind(kind):
    kind = text(kind)
    if kind in KINDS:
        return KINDS[kind]
    return title_case(kind.replace("_", " "))


def kind_icon(kind):
    kind = text(kind)
    if kind == "calendar_event":
        return "calendar-days"
    if kind == "meeting_transcript":
        return "message-square"
    if re.match(r"^(slack|discord|teams|google_chat|matrix)$", kind):
        return "hash"
    return "messages-square"


def account_icon(kind):
    kind = text(kind)
    if re.match(r"^(gmail|imap|fastmail|microsoft|outlook|mbox|emlx|pst)$", kind):
        return "mail"
    if re.match(r"^(whatsapp|imessage|apple_messages|google_messages|sms|beeper|google_voice)$", kind):
        return "message-square"
    if re.match(r"^(slack|discord|teams|google_chat|matrix)$", kind):
        return "hash"
    if re.match(r"^(gcal|calendar)$", kind):
        return "calendar-days"
    return "database"


# Initials and a hue (0..11) for an avatar, as gpui-kit's Avatar draws one.
def initials(name):
    words = [w for w in text(name).split(" ") if w]
    letters = "".join(w[:1] for w in words[:2])
    if len(letters) == 1:
        letters = ("?" if name is None else text(name))[:2]
    if letters == "":
        letters = "?"
    return letters.upper()


def hue(letters):
    return fnv(letters) % 12


def clean(value):
    if value is None:
        return ""
    value = text(value)
    for entity, char in (("&#39;", "'"), ("&quot;", '"'), ("&lt;", "<"), ("&gt;", ">"),
                         ("&nbsp;", " "), ("&amp;", "&")):
        value = value.replace(entity, char)
    value = re.sub(r"\s+", " ", value)
    return re.sub(r"^ | $", "", value)


# A readable plain rendering of an HTML body: reader.rs's html_to_text.
def html_to_text(html):
    html = re.sub(r"<(style|script|head)\b.*?</\1\s*>", "", html, flags=re.I | re.S)
    html = re.sub(r"<li\b[^>]*>", "\n• ", html, flags=re.I)
    html = re.sub(r"</?(br|p|div|tr|li|h[1-4]|blockquote|table)\b[^>]*>", "\n", html, flags=re.I)
    html = re.sub(r"</?(td|th)\b[^>]*>", "\t", html, flags=re.I)
    html = re.sub(r"<[^>]*>", "", html)
    for entity, char in (("&nbsp;", " "), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", '"'),
                         ("&#39;", "'"), ("&amp;", "&")):
        html = html.replace(entity, char)
    lines = []
    for line in html.split("\n"):
        line = re.sub(r"\s+$", "", line)
        line = re.sub(r"^\t+", "", line)
        line = re.sub(r"\t+", "  ", line)
        if line == "" and lines and lines[-1] == "":
            continue
        lines.append(line)
    return re.sub(r"^\s+|\s+$", "", "\n".join(lines))


def person(m):
    if text(m.get("name")) != "":
        return m["name"]
    if text(m.get("address")) != "":
        return m["address"]
    return "Unknown"
