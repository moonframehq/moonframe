//! `commonplace`: the notes engine from a shell, for agents and scripts.
//! Every command prints JSON. The vault is `--vault DIR`, else
//! `$COMMONPLACE_VAULT`, else the current folder.
//!
//! ```text
//! commonplace scan
//! commonplace search "spaced rep tag:learning" [--limit N] [--offset N]
//! commonplace switch srs
//! commonplace backlinks "notes/Spaced repetition.md"
//! commonplace outgoing | mentions | info "notes/Spaced repetition.md"
//! commonplace unresolved | orphans [FOLDER] | tags | tasks [--all]
//! commonplace list [FOLDER] [--by-title]
//! commonplace read PATH
//! commonplace rename FROM TO
//! commonplace import apple-notes [FOLDER] [--from-export DIR] [--dry-run]
//!     every note in Notes.app, into notes/ by default; with Notes.app's Markdown
//!     export in DIR, its bodies and attachments with Notes.app's folders and dates
//! commonplace bench NOTES        (builds a synthetic vault in a temp folder and times it)
//! ```

use {
    moonframe_notes::{Error, Order, Vault},
    serde::Serialize,
    std::{
        env,
        path::PathBuf,
        process::ExitCode,
        time::{Duration, Instant},
    },
};

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut flag = |name: &str| -> Option<String> {
        let at = args.iter().position(|arg| arg == name)?;
        args.remove(at);
        if at < args.len() {
            Some(args.remove(at))
        } else {
            None
        }
    };
    let vault = flag("--vault")
        .map(PathBuf::from)
        .or_else(|| env::var_os("COMMONPLACE_VAULT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let limit = flag("--limit")
        .and_then(|value| value.parse().ok())
        .unwrap_or(20);
    let offset = flag("--offset")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let export = flag("--from-export").map(PathBuf::from);
    let all = take(&mut args, "--all");
    let dry_run = take(&mut args, "--dry-run");
    let by_title = take(&mut args, "--by-title");
    let Some(command) = args.first().cloned() else {
        eprintln!(
            "usage: commonplace [--vault DIR] <scan|search|switch|backlinks|outgoing|mentions|info|unresolved|orphans|tags|tasks|list|read|rename|import|bench> ..."
        );
        return ExitCode::from(2);
    };
    let arg = |index: usize| args.get(index).cloned().unwrap_or_default();
    let result = if command == "bench" {
        bench(arg(1).parse().unwrap_or(10_000))
    } else {
        run(
            &vault,
            &command,
            &arg(1),
            &arg(2),
            offset,
            limit,
            all,
            by_title,
            dry_run,
            export,
        )
    };
    match result {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        },
        Err(error) => {
            println!("{}", serde_json::json!({ "error": error.to_string() }));
            ExitCode::FAILURE
        },
    }
}

fn take(args: &mut Vec<String>, name: &str) -> bool {
    let found = args.iter().position(|arg| arg == name);
    if let Some(at) = found {
        args.remove(at);
    }
    found.is_some()
}

fn json(value: impl Serialize) -> Result<String, Error> {
    serde_json::to_string_pretty(&value).map_err(|error| Error::Invalid(error.to_string()))
}

fn run(
    root: &PathBuf,
    command: &str,
    first: &str,
    second: &str,
    offset: usize,
    limit: usize,
    all: bool,
    by_title: bool,
    dry_run: bool,
    export: Option<PathBuf>,
) -> Result<String, Error> {
    // An import may be the vault's first content: make its folder.
    if command == "import" {
        std::fs::create_dir_all(root)?;
    }
    let mut vault = Vault::open(root)?;
    let report = vault.scan()?;
    let index = vault.index();
    match command {
        "scan" => json(report),
        "search" => json(index.search(first, offset, limit)?),
        "switch" => json(index.switch(first, limit)?),
        "backlinks" => json(index.backlinks(first, offset, limit)?),
        "outgoing" => json(index.outgoing(first)?),
        "mentions" => json(index.mentions(first, limit)?),
        "info" => json(index.info(first)?),
        "unresolved" => json(index.unresolved(limit)?),
        "orphans" => json(index.orphans(first, limit)?),
        "tags" => json(index.tags()?),
        "tasks" => json(index.tasks(all, offset, limit)?),
        "list" => json(index.list(
            first,
            if by_title {
                Order::Title
            } else {
                Order::Modified
            },
            offset,
            limit,
        )?),
        "read" => json(serde_json::json!({ "path": first, "text": vault.read(first)? })),
        "rename" => json(vault.rename(first, second)?),
        "import" if first == "markdown" => {
            let folder =
                export.ok_or_else(|| Error::Invalid("--from-export DIR is needed".to_string()))?;
            let into = if second.is_empty() {
                "notes"
            } else {
                second
            };
            json(summary(vault.import_markdown_export(
                &folder, "markdown", into, dry_run,
            )?))
        },
        "import" if first == "apple-notes" => {
            let into = if second.is_empty() {
                "notes"
            } else {
                second
            };
            json(summary(vault.import_apple_notes(
                into,
                export.as_deref(),
                dry_run,
                bar(),
            )?))
        },
        other => Err(Error::Invalid(format!("unknown command {other}"))),
    }
}

/// Builds a vault of `notes` synthetic notes (links, tags, tasks, a few
/// aliases, prose) and times a cold index, a re-scan, one changed file,
/// searches, the switcher and backlinks.
fn bench(notes: usize) -> Result<String, Error> {
    let temp = env::temp_dir().join(format!("commonplace-bench-{}", moonframe_notes::ulid()));
    let words = [
        "memory",
        "review",
        "spacing",
        "retrieval",
        "practice",
        "attention",
        "habit",
        "reading",
        "writing",
        "graph",
        "link",
        "note",
        "idea",
        "question",
        "curve",
        "learning",
        "focus",
        "sleep",
        "context",
        "theory",
    ];
    for i in 0..notes {
        let folder = ["notes", "wiki", "journal", "raw"][i % 4];
        let mut text = String::new();
        if i % 7 == 0 {
            text.push_str(&format!(
                "---\naliases: [N{i}]\ntags: [{}]\ntype: concept\n---\n",
                words[i % words.len()]
            ));
        }
        text.push_str(&format!("# Note {i}\n\n"));
        for p in 0..6 {
            for w in 0..40 {
                text.push_str(words[(i * 31 + p * 7 + w * 13) % words.len()]);
                text.push(' ');
            }
            text.push_str(&format!(
                "[[Note {}]] #{}\n\n",
                (i * 17 + p * 101) % notes,
                words[(i + p) % words.len()]
            ));
        }
        text.push_str(&format!("- [ ] task {i}\n- an idea ^b{i}\n"));
        let file = temp.join(folder).join(format!("Note {i}.md"));
        std::fs::create_dir_all(file.parent().unwrap_or(&temp))?;
        std::fs::write(file, text)?;
    }
    let timed = |run: &mut dyn FnMut() -> Result<(), Error>| -> Result<Duration, Error> {
        let start = Instant::now();
        run()?;
        Ok(start.elapsed())
    };
    let mut vault = Vault::open(&temp)?;
    let cold = timed(&mut || vault.scan().map(|_| ()))?;
    let warm = timed(&mut || vault.scan().map(|_| ()))?;
    let changed = temp.join("notes/Note 0.md");
    std::fs::write(
        &changed,
        "# Note 0\n\nRewritten, linking [[Note 1]] and [[Note 2]].\n",
    )?;
    let one = timed(&mut || vault.refresh("notes/Note 0.md"))?;
    let samples =
        |run: &mut dyn FnMut() -> Result<(), Error>| -> Result<(Duration, Duration), Error> {
            let mut times = Vec::new();
            for _ in 0..50 {
                times.push(timed(run)?);
            }
            times.sort();
            Ok((times[times.len() / 2], times[times.len() * 95 / 100]))
        };
    let index = vault.index();
    let queries = [
        "memory",
        "spacing retrieval",
        "curv",
        "\"attention habit\"",
        "tag:memory review",
        "path:wiki sleep",
    ];
    let mut q = 0;
    let search = samples(&mut || {
        q += 1;
        index.search(queries[q % queries.len()], 0, 20).map(|_| ())
    })?;
    let mut s = 0;
    let switch = samples(&mut || {
        s += 1;
        index
            .switch(["no", "note 12", "n7", "nt99"][s % 4], 20)
            .map(|_| ())
    })?;
    let backlinks = samples(&mut || index.backlinks("wiki/Note 1.md", 0, 50).map(|_| ()))?;
    let ms = |duration: Duration| (duration.as_secs_f64() * 1000.0 * 10.0).round() / 10.0;
    let size = std::fs::metadata(temp.join(".commonplace/index.sqlite"))
        .map(|meta| meta.len())
        .unwrap_or(0);
    let result = serde_json::json!({
        "notes": notes,
        "cold_index_ms": ms(cold),
        "rescan_unchanged_ms": ms(warm),
        "reindex_one_file_ms": ms(one),
        "search_ms": { "p50": ms(search.0), "p95": ms(search.1) },
        "switch_ms": { "p50": ms(switch.0), "p95": ms(switch.1) },
        "backlinks_ms": { "p50": ms(backlinks.0), "p95": ms(backlinks.1) },
        "index_mb": (size as f64 / 1_048_576.0 * 10.0).round() / 10.0,
    });
    drop(vault);
    let _ = std::fs::remove_dir_all(&temp);
    json(result)
}

/// An import's report with its long lists cut to their first ten names,
/// and how many each held.
fn summary(report: moonframe_notes::ImportReport) -> serde_json::Value {
    let mut value = serde_json::to_value(&report).unwrap_or_default();
    if let Some(object) = value.as_object_mut() {
        for key in ["created", "updated", "kept", "skipped"] {
            if let Some(list) = object
                .get_mut(key)
                .and_then(serde_json::Value::as_array_mut)
            {
                let count = list.len();
                list.truncate(10);
                object.insert(format!("{key}_count"), serde_json::json!(count));
            }
        }
        if let Some(list) = object
            .get_mut("joined")
            .and_then(|joined| joined.get_mut("export_only"))
            .and_then(serde_json::Value::as_array_mut)
        {
            list.truncate(20);
        }
    }
    value
}

/// Shows an import's progress on stderr: a bar redrawn in place in a
/// terminal, a line per phase otherwise.
fn bar() -> impl FnMut(moonframe_notes::Progress) {
    use {moonframe_notes::Progress, std::io::IsTerminal};
    let terminal = std::io::stderr().is_terminal();
    let mut last_phase = String::new();
    move |step| {
        let (phase, done, total, detail) = match step {
            Progress::Reading {
                done,
                total,
                folder,
            } => ("Reading Notes.app", done, total, folder),
            Progress::Matching => ("Matching the export", 0, 0, String::new()),
            Progress::Writing { done, total } => ("Writing notes", done, total, String::new()),
        };
        if !terminal {
            if phase != last_phase {
                eprintln!("{phase}…");
                last_phase = phase.to_string();
            }
            return;
        }
        if phase != last_phase && !last_phase.is_empty() {
            eprintln!();
        }
        last_phase = phase.to_string();
        const WIDTH: usize = 30;
        let filled = (done.min(total) * WIDTH).checked_div(total).unwrap_or(0);
        let detail: String = detail.chars().take(40).collect();
        let count = if total == 0 {
            String::new()
        } else {
            format!(" {done}/{total}")
        };
        eprint!(
            "\r\x1b[2K{phase} [{}{}]{count} {detail}",
            "█".repeat(filled),
            "░".repeat(WIDTH - filled)
        );
        if total > 0 && done >= total && phase == "Writing notes" {
            eprintln!();
        }
    }
}
