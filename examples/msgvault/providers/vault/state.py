# Where things are, what was read (the archive or the sample), what the user
# looks at, and what the user is told after opening or saving an attachment.
import os

# ---- where things are --------------------------------------------------

home = os.environ.get("HOME", "")
vault_home = os.environ.get("MSGVAULT_HOME") or home + "/.msgvault"
db_path = vault_home + "/msgvault.db"
cache_root = os.environ.get("XDG_CACHE_HOME") or home + "/.cache"
cache_dir = cache_root + "/moonframe/msgvault"
view_file = cache_dir + "/view.json"
html_dir = cache_dir + "/html"
# Pictures go where ui.image may read them: the app's image cache.
images_dir = cache_root + "/moonframe/images/msgvault"
blob_root = vault_home + "/attachments"
OPEN_LIMIT = 512 * 1024 * 1024   # largest attachment read, as msgvault-gpui
THUMB_LIMIT = 8 * 1024 * 1024    # largest picture shown inline

# What was read (the archive or the sample), and what the user looks at.
data, mode, read_at = None, "", 0
view = {"mailbox": "inbox", "account": 0, "query": "", "open": 0, "fkind": "all",
        "fquery": "", "file": "", "remote": {}}


def tilde(path):
    if home and path.startswith(home):
        return "~" + path[len(home):]
    return path


def make_dir(path):
    try:
        os.makedirs(path, exist_ok=True)
    except OSError:
        pass


def write_file(path, content):
    """Writes beside it then renames, so a reader never sees half a file."""
    try:
        with open(path + ".tmp", "wb") as out:
            out.write(content)
    except OSError:
        return False
    try:
        os.rename(path + ".tmp", path)
    except OSError:
        pass
    return True


# What the user is told after opening or saving an attachment.
notice_seq = 0
notice = None


def tell_user(ok, message):
    global notice_seq, notice
    notice_seq += 1
    notice = {"seq": notice_seq, "ok": bool(ok), "text": message}
