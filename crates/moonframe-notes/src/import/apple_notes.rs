//! Reading notes out of Notes.app (macOS), with JavaScript for Automation
//! run by `osascript`. The first run asks the user to let the app control
//! Notes (System Settings > Privacy & Security > Automation).
//!
//! Notes' HTML holds pasted pictures inline, so a folder's bodies read at
//! once can run osascript out of memory. Bodies are therefore read one note
//! at a time and written out in small batches, and a run that has Notes'
//! own Markdown export reads no bodies at all, only a short plain-text
//! sample of each note to match it by ([`Mode::Sample`]); the bodies of
//! notes missing from the export are read afterwards ([`bodies`]).

use {
    super::SourceNote,
    crate::Error,
    serde::Deserialize,
    std::{
        collections::HashMap,
        fs,
        io::{BufRead, BufReader},
        path::Path,
        process::{Command, Stdio},
    },
};

/// Folders that hold deleted notes, by the names Notes gives them.
const DELETED: &[&str] = &[
    "Recently Deleted",
    "Suppressions récentes",
    "Zuletzt gelöscht",
    "Eliminados recientemente",
    "Eliminati di recente",
];

/// Notes per file the script writes.
const BATCH: usize = 25;

/// How much of a note's plain text a sample keeps.
const SAMPLE: usize = 3000;

/// What the script reads of each note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Its HTML body.
    Full,
    /// The start of its plain text, to match it to an exported file.
    Sample,
}

const SCRIPT: &str = r#"
function run(argv) {
    ObjC.import('Foundation');
    const out = argv[0];
    const skip = JSON.parse(argv[1]);
    const mode = argv[2];
    const ids = argv[3] ? JSON.parse(ObjC.unwrap($.NSString.stringWithContentsOfFileEncodingError(argv[3], $.NSUTF8StringEncoding, null))) : null;
    const batch = BATCH_SIZE, sample = SAMPLE_SIZE;
    const Notes = Application('Notes');
    let files = 0, total = 0;
    function write(value) {
        const text = $.NSString.alloc.initWithUTF8String(JSON.stringify(value));
        const name = out + '/' + ('00000' + files).slice(-5) + '.json';
        files += 1;
        text.writeToFileAtomicallyEncodingError(name, true, $.NSUTF8StringEncoding, null);
    }
    function say(value) { console.log(JSON.stringify(value)); }
    function iso(date) {
        try { return date ? date.toISOString() : null; } catch (e) { return null; }
    }
    function body(note) {
        try {
            if (mode === 'sample') {
                const text = note.plaintext();
                return text ? text.slice(0, sample) : '';
            }
            return note.body();
        } catch (e) { return null; }
    }
    if (ids) {
        // Bodies of the given notes only.
        say({ total: ids.length });
        let list = [];
        ids.forEach(function (id) {
            let html = null;
            try { html = Notes.notes.byId(id).body(); } catch (e) { html = null; }
            list.push({ id: id, html: html });
            total += 1;
            if (list.length >= batch) { write({ bodies: list }); list = []; say({ read: total, folder: '' }); }
        });
        if (list.length) { write({ bodies: list }); say({ read: total, folder: '' }); }
        return JSON.stringify({ files: files, notes: total, accounts: [] });
    }
    let all = 0;
    try { all = Notes.notes.length; } catch (e) { all = 0; }
    say({ total: all });
    const seen = {};
    function walk(account, folder, path) {
        const id = folder.id();
        if (seen[id]) return;
        seen[id] = true;
        const name = folder.name();
        if (path.length === 0 && skip.indexOf(name) >= 0) return;
        const here = path.concat([name]);
        const notes = folder.notes;
        const count = notes.length;
        if (count > 0) {
            // Small values in bulk; bodies one note at a time.
            const ids = notes.id(), names = notes.name();
            const created = notes.creationDate(), modified = notes.modificationDate();
            let locked = [];
            try { locked = notes.passwordProtected(); } catch (e) { locked = ids.map(function () { return false; }); }
            let list = [];
            for (let i = 0; i < count; i++) {
                const text = locked[i] ? null : body(notes[i]);
                list.push({ id: ids[i], title: names[i], created: iso(created[i]), modified: iso(modified[i]), locked: !!locked[i], text: text });
                total += 1;
                if (list.length >= batch) {
                    write({ account: account, folder: here, notes: list });
                    list = [];
                    say({ read: total, folder: here.join(' / ') });
                }
            }
            if (list.length) { write({ account: account, folder: here, notes: list }); }
            say({ read: total, folder: here.join(' / ') });
        }
        folder.folders().forEach(function (sub) { walk(account, sub, here); });
    }
    const accounts = Notes.accounts();
    accounts.forEach(function (account) {
        const name = account.name();
        account.folders().forEach(function (folder) { walk(name, folder, []); });
    });
    return JSON.stringify({ files: files, notes: total, accounts: accounts.map(function (a) { return a.name(); }) });
}
"#;

#[derive(Deserialize)]
struct Batch {
    #[serde(default)]
    account: String,
    #[serde(default)]
    folder: Vec<String>,
    #[serde(default)]
    notes: Vec<Read>,
    #[serde(default)]
    bodies: Vec<Body>,
}

#[derive(Deserialize)]
struct Read {
    id: String,
    title: String,
    created: Option<String>,
    modified: Option<String>,
    locked: bool,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Body {
    id: String,
    html: Option<String>,
}

#[derive(Deserialize)]
struct Summary {
    accounts: Vec<String>,
}

/// Runs the script into `scratch` (emptied first) and returns what it
/// wrote, batch by batch, with its summary. `ids` asks for those notes'
/// bodies only.
fn run(
    scratch: &Path,
    mode: Mode,
    ids: Option<&[String]>,
    mut progress: impl FnMut(usize, usize, &str),
) -> Result<(Vec<Batch>, Summary), Error> {
    if !cfg!(target_os = "macos") {
        return Err(Error::Invalid("Notes.app is only on macOS".to_string()));
    }
    let _ = fs::remove_dir_all(scratch);
    fs::create_dir_all(scratch)?;
    let script = scratch.join("export.js");
    let source = SCRIPT
        .replace("BATCH_SIZE", &BATCH.to_string())
        .replace("SAMPLE_SIZE", &SAMPLE.to_string());
    fs::write(&script, source)?;
    let skip = serde_json::to_string(DELETED).unwrap_or_else(|_| "[]".to_string());
    let ids_file = scratch.join("ids.json");
    if let Some(ids) = ids {
        fs::write(&ids_file, serde_json::to_vec(ids).unwrap_or_default())?;
    }
    let out = scratch.join("out");
    fs::create_dir_all(&out)?;
    let mut child = Command::new("osascript")
        .arg("-l")
        .arg("JavaScript")
        .arg(&script)
        .arg(&out)
        .arg(skip)
        .arg(if mode == Mode::Sample {
            "sample"
        } else {
            "full"
        })
        .arg(if ids.is_some() {
            ids_file.display().to_string()
        } else {
            String::new()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // The script's progress, as it reads; anything else it says is an error.
    let mut total = 0;
    let mut said = String::new();
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines() {
            let line = line?;
            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(value) if value.get("total").is_some() => {
                    total = value["total"].as_u64().unwrap_or(0) as usize;
                    progress(0, total, "");
                },
                Ok(value) if value.get("read").is_some() => {
                    let read = value["read"].as_u64().unwrap_or(0) as usize;
                    progress(
                        read,
                        total.max(read),
                        value["folder"].as_str().unwrap_or(""),
                    );
                },
                _ => {
                    said.push_str(&line);
                    said.push('\n');
                },
            }
        }
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let _ = fs::remove_dir_all(scratch);
        return Err(Error::Invalid(explain(said.trim())));
    }
    let summary: Summary = serde_json::from_slice(&output.stdout)
        .map_err(|error| Error::Invalid(format!("Notes.app answered oddly: {error}")))?;
    let mut files: Vec<_> = fs::read_dir(&out)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    let mut batches = Vec::with_capacity(files.len());
    for file in files {
        let batch: Batch = serde_json::from_slice(&fs::read(&file)?)
            .map_err(|error| Error::Invalid(format!("{}: {error}", file.display())))?;
        batches.push(batch);
    }
    let _ = fs::remove_dir_all(scratch);
    Ok((batches, summary))
}

/// Every note in Notes.app: with its HTML body ([`Mode::Full`]), or only a
/// plain-text sample of it ([`Mode::Sample`], in `plain`). `progress` hears
/// how many notes have been read, of about how many, and the folder.
pub(crate) fn export(
    scratch: &Path,
    mode: Mode,
    progress: impl FnMut(usize, usize, &str),
) -> Result<Vec<SourceNote>, Error> {
    let (batches, summary) = run(scratch, mode, None, progress)?;
    let several_accounts = summary.accounts.len() > 1;
    let mut out = Vec::new();
    for batch in batches {
        let mut path = Vec::new();
        if several_accounts {
            path.push(batch.account.clone());
        }
        path.extend(batch.folder.iter().cloned());
        for note in batch.notes {
            let (html, plain) = match mode {
                Mode::Full => (note.text, None),
                Mode::Sample => (None, note.text),
            };
            out.push(SourceNote {
                id: note.id,
                title: note.title,
                folder: path.clone(),
                html,
                plain,
                markdown: None,
                attachments: Vec::new(),
                created: note.created,
                modified: note.modified,
                locked: note.locked,
            });
        }
    }
    Ok(out)
}

/// The HTML bodies of the notes with `ids`, one at a time.
pub(crate) fn bodies(
    scratch: &Path,
    ids: &[String],
    progress: impl FnMut(usize, usize, &str),
) -> Result<HashMap<String, String>, Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let (batches, _) = run(scratch, Mode::Full, Some(ids), progress)?;
    Ok(batches
        .into_iter()
        .flat_map(|batch| batch.bodies)
        .filter_map(|body| body.html.map(|html| (body.id, html)))
        .collect())
}

/// What to tell the user when osascript fails.
fn explain(message: &str) -> String {
    if message.contains("-1743")
        || message.contains("Not authorized")
        || message.contains("not allowed")
    {
        "Commonplace may not control Notes yet. Allow it in System Settings > Privacy & Security > Automation, then import again.".to_string()
    } else if message.contains("-600") || message.contains("isn't running") {
        "Notes.app could not be started.".to_string()
    } else {
        format!("Reading Notes.app failed: {message}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_a_refused_permission() {
        assert!(
            explain("execution error: Not authorized to send Apple events to Notes. (-1743)")
                .contains("Automation")
        );
    }

    #[test]
    fn the_script_gets_its_sizes() {
        let source = SCRIPT
            .replace("BATCH_SIZE", &BATCH.to_string())
            .replace("SAMPLE_SIZE", &SAMPLE.to_string());
        assert!(source.contains("const batch = 25, sample = 3000;"));
    }
}
